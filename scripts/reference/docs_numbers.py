#!/usr/bin/env python3
"""Computes the numbers of the documentation that come from expectations and simulations.

Most numbers in the documentation are outputs of dima runs, fixed by the tests. Some are not:
how far one run of the paper's random procedure strays from the exact expectation DiMA reports,
how often DiMA v5.0.9's per-run negative-intercept rule replaces an intercept and how much that
raises its averages, how the similarity-weighted values change with the number of sequences,
and the values of Wu 2025's ToBRFV samples. This script computes them and writes them between
markers in the pages that show them, and in the notes of scripts/validate_paper.sh:

    <!-- docs-numbers:NAME:start -->...<!-- docs-numbers:NAME:end -->

A region holds only what is computed here (a table, or a sentence whose numbers are computed);
the prose around it is written by hand. A *block* region stands on its own lines and holds whole
paragraphs or tables; an *inline* region starts at the end of a line of prose (a line never
starts with the marker, which would end the paragraph) and its text is wrapped at 100 columns; a
*code* region in validate_paper.sh holds Python assignments. `--check` prints a unified diff and
exits with 1 when a region is out of date (CI); without it the regions are rewritten in place.

Standard library only (Python >= 3.10). It imports the reference implementation
(dima_reference.py, in this directory) for parsing, the support classes, entropies and the
pairwise similarity code; it inserts its own directory into sys.path and writes no bytecode, so
`python3 -I -B scripts/reference/docs_numbers.py` works. About a minute on four cores (the
fixed list of tasks below runs in a process pool; the results do not depend on the number of
processes).

The procedures, as these pages read them
----------------------------------------
* The paper's random procedure (entropy.md, "The paper's correction (Fig. 2)"): for a position
  with support N > T, subsample sizes n = ceil(R N / 100) for R = R0, R0 + 5, ..., 95, where R0
  is 50, or ceil(100 T / N) rounded up to a multiple of 5 when that is 50 or more; one subsample
  of n k-mers drawn with replacement per size, independently; the points (1/n, plug-in entropy)
  and (1/N, H) fitted by least squares; the intercept S is the estimate. The observed entropy H
  is used when N <= T or with fewer than five points. Its "standard deviation" here is that of S
  (before any negative-intercept rule), and a run's mean is the mean of S over the positions
  with support above T, positions being resampled independently.
* DiMA v5.0.9's per-run procedure (entropy.md "The negative-intercept rule"; differences.md,
  "Method"; CHANGELOG 1.0.0, "Methodology"): the same grid, but sizes floor(R N / 100), and each
  run whose intercept is negative reports the observed entropy H instead. v5.0.9 computes R0 in
  floating point, which equals the exact value for T = 30 and 100 (asserted below for every N
  used); it corrects when N >= T rather than N > T, which changes nothing, because N = T leaves
  only the 100% point. Its averages are means over the positions with support above T, as
  DiMA's; "raises the average by x%" compares v5.0.9's expected value with DiMA's.
* Wu 2025's Fig. 1b, quasispecies 2: a master sequence at 90% and four variants at 4%, 4%, 1%
  and 1% carrying 2, 4, 2 and 4 substitutions (the red triangles), drawn at distinct sites and
  read so: k = 12, the shortest length that holds the 12 substitutions at distinct sites
  (FIG1B_K), so that two variants differ at the sum of their substitutions.
* ToBRFV: the unique sequences of each sample (the distinct 120-mers; tests/data/README.md);
  their pairwise Hamming distances are averaged over the pairs of distinct unique sequences, the
  reading that gives Wu's 2.12 and 2.80 (Section 3.3).

What is computed, and how
-------------------------
"Exact" means computed from the exact distributions (binomial, multinomial, multivariate
hypergeometric), not simulated; binomial tails below 1e-20 of the mode's term are dropped, and
where more is left out (meet in the middle) the omitted probability is checked against the
precision printed.

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
* The random procedure's standard deviations: the subsamples are independent, so Var(S) =
  sum_j w_j^2 Var(H_n_j) with the least-squares weights w_j. Var(H_n) needs the covariances of
  the counts of every pair of variants; they come from the Lancaster expansion of the
  trinomial in orthonormal Krawtchouk polynomials (canonical correlations rho^k, rho =
  -sqrt(p_i p_j / ((1 - p_i)(1 - p_j)))), which makes the sum over pairs separable; the series is
  summed until a Cauchy-Schwarz bound on the rest is below 1e-13 of the variance. Checked
  against enumerating every multinomial outcome of toy profiles.
* v5.0.9's fallback rates at positions with two k-mers: exact, by meet-in-the-middle over the
  independent subsample counts (the probability left out, at most 1e-6, bounds the error; checked
  against enumerating every outcome of a small case). With several different minor variants,
  and v5.0.9's averages over the MERS-CoV datasets: seeded Monte Carlo that simulates only the
  rule's effect, E[(H - S) 1{S < 0}], added to the exact expectation E[S] (a control variate).
  The Monte Carlo is checked against the exact two-k-mer results.
* HIV-1 pol: v5.0.9's average differs from DiMA's mean by the exact effect of its rounding plus
  the rule's effect, bounded above per position by min(Cantelli, McDiarmid) for P(S < 0) and
  min(Scarf, McDiarmid) for E[max(-S, 0)], with the exact mean and variance of S and McDiarmid's
  bounded differences (one draw changes a plug-in entropy of n draws by at most
  (n log2 n - (n - 1) log2(n - 1)) / n).
* Population study: two-variant populations (minor frequency phi) sampled N times; the error of
  DiMA's exact expectation, of Miller-Madow and of the observed entropy against the
  population's entropy, averaged exactly over the binomial number of minor sequences; v5.0.9's
  per-run error by seeded Monte Carlo of the rule's effect (stratified on at least one minor
  sequence) added to its exact expectation.

Monte Carlo uses only random.Random(seed).random(), whose stream is the same on CPython 3.10 to
3.13 and on every platform, with the seeds and run counts below; binomial counts are drawn by
inverting cumulative tables built with + - * / only. The texts state the run counts and the
95% half-widths (1.96 standard errors), rounded up; where a value is printed to a single precision
(the v5.0.9 means), the half-width is checked to stay within it.

Usage: python3 scripts/reference/docs_numbers.py [--check]
"""

from __future__ import annotations

import argparse
import concurrent.futures
import difflib
import math
import operator
import os
import random
import re
import sys
import time
from bisect import bisect_left, bisect_right
from collections import Counter
from fractions import Fraction
from functools import lru_cache
from itertools import accumulate, combinations, product
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
VALIDATE = ROOT / "scripts/validate_paper.sh"

DATASETS = {
    "mers_aa": "samples/mers_spike_aa.fasta",
    "mers_nt": "tests/data/MERS_Spike_nt.fasta.bz2",
    "hiv_dna": "tests/data/HIV1_FLT_2022_pol_DNA.fasta.bz2",
    "hiv_pro": "tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2",
}
TOBRFV = (("tm-2", "tests/data/tobrfv_tm2.fasta.bz2"), ("Tm-2²", "tests/data/tobrfv_tm22.fasta.bz2"))

#: The paper's Fig. 1C value at MERS-CoV spike position 25 (T = 30).
PAPER_FIG1C = 1.38
#: Table 1: subset sizes at MERS-CoV spike position 25 (its support is 170).
TABLE1_N = (10, 25, 50, 100, 170)
#: Table 2: sample sizes (with replacement).
TABLE2_N = (10, 25, 50, 100)
#: Wu 2025, Fig. 1b, quasispecies 2: (frequency in %, substitutions from the master).
FIG1B = ((90, 0), (4, 2), (4, 4), (1, 2), (1, 4))
#: The shortest k-mer length that holds Fig. 1b's 12 substitutions at distinct sites.
FIG1B_K = 12
#: Positions with every other sequence carrying the Index: N on the full eleven-point grid,
#: multiples of 20 so that R N / 100 is an integer and v5.0.9's sizes equal the paper's.
FALLBACK_N = (200, 300, 500, 1000, 2000, 3000)
#: Numbers of sequences that differ from the Index.
FALLBACK_MINOR = (1, 2, 5)
#: Few-point cases of one differing sequence (T = 100).
FEW_POINTS_N = (171, 130)
#: Population study grid.
POP_N = (171, 200, 300, 500, 1000, 2000, 3000)
POP_PHI = ((1, 2000), (1, 1000), (1, 500), (1, 200), (1, 100), (1, 50), (1, 20))
POP_T = (30, 100)

#: Monte Carlo: (seed, runs per task, tasks). Seeds of the tasks are seed * 1000 + task index.
MC_MERS = {
    ("mers_aa", 100): (11, 250, 8),
    ("mers_aa", 30): (12, 250, 8),
    ("mers_nt", 100): (13, 25, 8),
}
MC_FALLBACK = (21, 100_000)  # seed, runs per (N, number of different minor variants)
MC_POPULATION = (31, 100_000)  # seed, runs per population cell

LN2 = math.log(2.0)
#: Binomial windows drop the terms below this fraction of the mode's.
CUT = 1e-20

# ---------------------------------------------------------------------------------------------
# Shared numerics
# ---------------------------------------------------------------------------------------------


def binomial_window(n: int, a: int, b: int, cut: float = CUT) -> tuple[int, list[float]]:
    """(lo, P) with P[i] = P(X = lo + i) for X ~ Binomial(n, a / (a + b)), a, b >= 0 integers.

    Walks out from the mode with the exact ratios of consecutive terms (one rounded division
    each, as dima_reference.binomial_weights) and stops where a term falls below ``cut`` of the
    mode's; normalised with math.fsum. Only + - * / are used, so the result is the same on
    every platform.
    """
    if a == 0:
        return 0, [1.0]
    if b == 0:
        return n, [1.0]
    mode = min(n, ((n + 1) * a) // (a + b))
    up: list[float] = []
    w, x = 1.0, mode
    while x < n:
        w *= ((n - x) * a) / ((x + 1) * b)
        x += 1
        if w < cut:
            break
        up.append(w)
    down: list[float] = []
    w, x = 1.0, mode
    while x > 0:
        w *= (x * b) / ((n - x + 1) * a)
        x -= 1
        if w < cut:
            break
        down.append(w)
    down.reverse()
    weights = down + [1.0] + up
    total = math.fsum(weights)
    return mode - len(down), [v / total for v in weights]


_H_TABLES: dict[int, tuple[float, ...]] = {}


def h_table(n: int) -> tuple[float, ...]:
    """(h(0), ..., h(n)), h(x) = -(x/n) log2(x/n): the reference implementation's table."""
    t = _H_TABLES.get(n)
    if t is None:
        t = R._h_table(n)
        _H_TABLES[n] = t
    return t


_H2_TABLES: dict[int, list[float]] = {}


def h2_table(n: int) -> list[float]:
    """Plug-in entropy (bits) of a two-k-mer sample of n with x copies of one: h(x) + h(n - x)."""
    t = _H2_TABLES.get(n)
    if t is None:
        h = h_table(n)
        t = [h[x] + h[n - x] for x in range(n + 1)]
        _H2_TABLES[n] = t
    return t


def entropy(counts: list[int]) -> float:
    """Plug-in entropy in bits, correctly rounded (the reference implementation)."""
    return R.plug_in_entropy(R.multiplicities(counts), sum(counts))


def grid_sizes(n_obs: int, threshold: int, floor: bool = False) -> list[int]:
    """Subsample sizes of the grid: ceil(R N / 100) (the paper), or floor(R N / 100) (v5.0.9)."""
    x = (100 * threshold + n_obs - 1) // n_obs
    start = 50 if x < 50 else ((x + 4) // 5) * 5
    if floor:
        return [(r * n_obs) // 100 for r in range(start, 100, 5)]
    return [(r * n_obs + 99) // 100 for r in range(start, 100, 5)]


@lru_cache(maxsize=None)
def design(n_obs: int, threshold: int, floor: bool = False) -> tuple[tuple[int, ...], tuple[float, ...]] | None:
    """(sizes, weights) of the regression, or None when the observed entropy is reported.

    The intercept is sum_j w_j y_j over the subsample points and the 100% point (the last
    weight); the weights are computed exactly as fractions, w_j = 1/m - u_bar (u_j - u_bar) /
    sum_l (u_l - u_bar)^2 with u = 1/n (entropy.md, eq. "ols"), then rounded once.
    """
    if n_obs <= threshold:
        return None
    sizes = grid_sizes(n_obs, threshold, floor)
    if len(sizes) + 1 < 5:
        return None
    us = [Fraction(1, n) for n in sizes] + [Fraction(1, n_obs)]
    m = len(us)
    u_bar = sum(us) / m
    suu = sum((u - u_bar) ** 2 for u in us)
    if suu == 0:
        return None
    weights = tuple(float(Fraction(1, m) - u_bar * (u - u_bar) / suu) for u in us)
    return tuple(sizes), weights


def v509_start(n_obs: int, threshold: int) -> int:
    """v5.0.9's grid start, ceil((T / N) * 100) in floating point (entropy.md: T = 55, N = 100
    gives 56 there instead of 55)."""
    return math.ceil(threshold / n_obs * 100)


def check_v509_start(n_obs: int, threshold: int) -> None:
    """The readings use the exact grid start for v5.0.9 too: check that they agree."""
    if v509_start(n_obs, threshold) != (100 * threshold + n_obs - 1) // n_obs:
        raise RuntimeError(f"v5.0.9's floating-point grid start differs at T = {threshold}, N = {n_obs}")


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
    k, positions = read_alignment(DATASETS["mers_aa"])
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
# The paper's random procedure: exact moments
# ---------------------------------------------------------------------------------------------


def g(x: int) -> float:
    """g(x) = x log2 x, g(0) = g(1) = 0: the plug-in entropy of n draws is log2 n - sum g(X_i) / n."""
    return x * math.log2(x) if x > 1 else 0.0


def g_values(lo: int, size: int) -> list[float]:
    """g(x) for x = lo .. lo + size - 1."""
    return [g(x) for x in range(lo, lo + size)]


def plugin_moments(mult: dict[int, int], n_obs: int, n: int, rel: float = 1e-13) -> tuple[float, float]:
    """(E, Var) of the plug-in entropy (bits) of n k-mers drawn with replacement from a position
    with count classes ``mult`` ({count: number of k-mers}) and support ``n_obs``.

    H_n = log2 n - G / n with G = sum_i g(X_i), X ~ Multinomial(n, c_i / N). Var(G) = sum_i
    Var(g(X_i)) + sum_{i != j} Cov(g(X_i), g(X_j)). The counts of two variants are trinomial,
    whose joint distribution has the Lancaster expansion b_i(x) b_j(y) sum_k rho_ij^k Q_ik(x)
    Q_jk(y) in the orthonormal Krawtchouk polynomials Q of the binomial marginals, with
    rho_ij = -r_i r_j, r = sqrt(p / (1 - p)). So Cov = sum_{k >= 1} (-1)^k (r_i r_j)^k a_ik a_jk,
    a_ik = E[g(X_i) Q_ik(X_i)], and the sum over pairs separates per k. The tail beyond k is at
    most sum_{i != j} (r_i r_j)^(k+1) t_i t_j, with t_i^2 = Var(g(X_i)) - sum_{l <= k} a_il^2
    (Cauchy-Schwarz and Parseval); the series stops when that is below ``rel`` of the variance.
    A variant with p > 1/2 (r > 1) is paired with the others through rho directly, so no power
    overflows. Two variants: G is a function of one count.
    """
    classes = sorted(mult.items())
    n_var = sum(f for _, f in classes)
    if n_var == 1:
        return 0.0, 0.0
    if n_var == 2:
        c = classes[0][0]
        lo, P = binomial_window(n, c, n_obs - c)
        gs = [g(x) + g(n - x) for x in range(lo, lo + len(P))]
        mean = math.fsum(map(operator.mul, P, gs))
        var = math.fsum(p * (v - mean) ** 2 for p, v in zip(P, gs, strict=True))
        return math.log2(n) - mean / n, var / (n * n)
    info = []
    for c, f in classes:
        lo, P = binomial_window(n, c, n_obs - c)
        gs = g_values(lo, len(P))
        mu = math.fsum(map(operator.mul, P, gs))
        dev = [v - mu for v in gs]
        pg = list(map(operator.mul, P, dev))
        var = math.fsum(map(operator.mul, pg, dev))
        p, q = c / n_obs, (n_obs - c) / n_obs
        info.append({
            "f": f, "p": p, "q": q, "r": math.sqrt(c / (n_obs - c)), "mu": mu, "var": var, "tail": var,
            "pg": pg, "xs": [x - n * p for x in range(lo, lo + len(P))],
            "q0": [0.0] * len(P), "q1": [1.0] * len(P), "a": 0.0,  # Q_{-1} = 0, Q_0 = 1
            "dominant": 2 * c > n_obs,
        })
    mean_g = math.fsum(cl["f"] * cl["mu"] for cl in info)
    diag = math.fsum(cl["f"] * cl["var"] for cl in info)
    dom = next((cl for cl in info if cl["dominant"]), None)
    rest = [cl for cl in info if not cl["dominant"]]
    cross = []
    k = 0
    while True:
        k += 1
        for cl in info:
            # sqrt(c_k) Q_k = (x - b_{k-1}) Q_{k-1} - sqrt(c_{k-1}) Q_{k-2}, b_j = (n - j) p + j q,
            # c_j = j (n - j + 1) p q: the three-term recurrence of the binomial's orthonormal
            # (Krawtchouk) polynomials; xs holds x - n p, so x - b_{k-1} = xs - shift.
            p, q = cl["p"], cl["q"]
            s_k = math.sqrt(k * (n - k + 1) * p * q)
            shift = (n - k + 1) * p + (k - 1) * q - n * p
            s_prev = math.sqrt((k - 1) * (n - k + 2) * p * q)
            new = [((x - shift) * v1 - s_prev * v0) / s_k for x, v1, v0 in zip(cl["xs"], cl["q1"], cl["q0"], strict=True)]
            cl["q0"], cl["q1"] = cl["q1"], new
            a = math.fsum(map(operator.mul, cl["pg"], new))
            cl["a"] = a
            cl["tail"] = max(cl["tail"] - a * a, 0.0)
        xs = [(cl["f"], cl["r"] ** k * cl["a"]) for cl in rest]
        total = math.fsum(f * x for f, x in xs)
        term = [f * x * (total - x) for f, x in xs]
        ys = [(cl["f"], cl["r"] ** (k + 1) * math.sqrt(cl["tail"])) for cl in rest]
        y_total = math.fsum(f * y for f, y in ys)
        bound = [f * y * (y_total - y) for f, y in ys]
        if dom is not None:
            term.append(2 * dom["a"] * math.fsum(cl["f"] * (dom["r"] * cl["r"]) ** k * cl["a"] for cl in rest))
            bound.append(2 * math.sqrt(dom["tail"]) * math.fsum(cl["f"] * (dom["r"] * cl["r"]) ** (k + 1) * math.sqrt(cl["tail"]) for cl in rest))
        value = math.fsum(term)
        cross.append(-value if k % 2 else value)
        if math.fsum(bound) <= rel * diag:
            break
        if k >= n:
            break
        if k >= 500:
            raise RuntimeError(f"Krawtchouk series did not converge (n = {n}, profile {mult})")
    var_g = diag + math.fsum(cross)
    return math.log2(n) - mean_g / n, var_g / (n * n)


def intercept_moments(counts: list[int], threshold: int, floor: bool = False) -> dict:
    """Mean, variance and McDiarmid constant of the procedure's intercept S at a position."""
    n_obs = sum(counts)
    h_full = entropy(counts)
    d = design(n_obs, threshold, floor)
    if d is None or len(counts) == 1:
        return {"mean": h_full, "var": 0.0, "mcd": 0.0, "H": h_full}
    sizes, weights = d
    mult = dict(Counter(counts))
    means, variances, mcd = [weights[-1] * h_full], [], []
    for n, w in zip(sizes, weights[:-1], strict=True):
        e, v = plugin_moments(mult, n_obs, n)
        means.append(w * e)
        variances.append(w * w * v)
        c = (g(n) - g(n - 1)) / n  # one draw moves the plug-in entropy by at most this
        mcd.append(n * (w * c) ** 2)
    return {"mean": math.fsum(means), "var": math.fsum(variances), "mcd": math.fsum(mcd), "H": h_full}


def rule_effect_bound(mom: dict) -> float:
    """Upper bound on E[(H - S) 1{S < 0}] from the mean, variance and McDiarmid constant of S."""
    mu, var, v = mom["mean"], mom["var"], mom["mcd"]
    if var == 0.0:
        return 0.0
    p = var / (var + mu * mu)  # Cantelli
    e = (math.sqrt(var + mu * mu) - mu) / 2  # Scarf
    if v > 0:
        p = min(p, math.exp(-2 * mu * mu / v))
        e = min(e, math.sqrt(math.pi * v / 8) * math.erfc(mu * math.sqrt(2 / v)))
    return mom["H"] * p + e


def enumerated_variance(counts: list[int], n: int) -> tuple[float, float]:
    """(E, Var) of the plug-in entropy of n draws with replacement, by enumeration."""
    n_obs = sum(counts)
    h = h_table(n)
    probs, values = [], []
    for xs in product(range(n + 1), repeat=len(counts)):
        if sum(xs) != n:
            continue
        coef = math.factorial(n)
        for x in xs:
            coef //= math.factorial(x)
        probs.append(coef * math.prod(c ** x for c, x in zip(counts, xs, strict=True)) / n_obs ** n)
        values.append(math.fsum(h[x] for x in xs))
    mean = math.fsum(p * v for p, v in zip(probs, values, strict=True))
    return mean, math.fsum(p * (v - mean) ** 2 for p, v in zip(probs, values, strict=True))


def check_moments() -> None:
    for counts, n in (([5, 3, 2, 1, 1], 8), ([7, 2, 2, 1], 9), ([3, 3, 3], 6), ([9, 1], 7), ([4, 4], 6),
                      ([40, 2, 1, 1], 6), ([4, 4, 1, 1, 1, 1], 10)):
        e, v = plugin_moments(dict(Counter(counts)), sum(counts), n)
        e2, v2 = enumerated_variance(counts, n)
        if abs(e - e2) > 1e-12 or abs(v - v2) > 1e-12 * max(1.0, v2):
            raise RuntimeError(f"plugin_moments({counts}, {n}) = {(e, v)}, enumeration {(e2, v2)}")


def random_procedure(numbers: list[int], counts: list[list[int]], threshold: int) -> list[dict]:
    """intercept_moments of the paper's procedure (ceil) and of v5.0.9's (floor) at positions."""
    out = []
    for number, cs in zip(numbers, counts, strict=True):
        ceil = intercept_moments(cs, threshold, False)
        floor = intercept_moments(cs, threshold, True)
        out.append({"position": number, "ceil": ceil, "floor": floor, "bound": rule_effect_bound(floor)})
    return out


# ---------------------------------------------------------------------------------------------
# DiMA v5.0.9's per-run rule
# ---------------------------------------------------------------------------------------------


@lru_cache(maxsize=None)
def two_kmer_intercept(n_obs: int, minor: int, threshold: int, floor: bool) -> float:
    """Exact expected intercept at a position with two k-mers (counts N - a, a)."""
    h_full = h2_table(n_obs)[minor]
    d = design(n_obs, threshold, floor)
    if d is None:
        return h_full
    sizes, weights = d
    parts = [weights[-1] * h_full]
    for n, w in zip(sizes, weights[:-1], strict=True):
        lo, P = binomial_window(n, minor, n_obs - minor)
        t = h2_table(n)
        parts.append(w * math.fsum(p * t[lo + i] for i, p in enumerate(P)))
    return math.fsum(parts)


def rule_effect_exact(n_obs: int, minor: int, threshold: int, floor: bool = True, cut: float = 1e-13) -> dict:
    """P(S < 0) and E[(H - S) 1{S < 0}] of one run at a position with two k-mers, exactly.

    S = c + sum_j w_j y_j with independent y_j = H2(X_j / n_j), X_j ~ Binomial(n_j, a / N).
    Meet in the middle: the subsample points are split into two halves, every combination of
    each half with probability >= ``cut`` is listed, one half sorted, and for each combination
    of the other the probability and first moment of S < 0 are read from prefix sums. The
    combinations left out have total probability ``omitted``, which bounds the error.
    """
    d = design(n_obs, threshold, floor)
    if d is None:
        raise RuntimeError("rule_effect_exact: no regression")
    sizes, weights = d
    h_full = h2_table(n_obs)[minor]
    const = weights[-1] * h_full
    terms = []
    for n, w in zip(sizes, weights[:-1], strict=True):
        lo, P = binomial_window(n, minor, n_obs - minor)
        t = h2_table(n)
        terms.append([(w * t[lo + i], p) for i, p in enumerate(P) if p >= cut * 1e-2])

    def combine(part):
        cur = [(0.0, 1.0)]
        for atoms in part:
            cur = [(s + v, p * q) for s, p in cur for v, q in atoms if p * q >= cut]
        return cur

    half = len(terms) // 2
    left = combine(terms[:half])
    right = sorted(combine(terms[half:]))
    values = [v for v, _ in right]
    cum_p = [0.0] + list(accumulate(p for _, p in right))
    cum_v = [0.0] + list(accumulate(v * p for v, p in right))
    p_neg, e_neg = [], []
    for a, pa in left:
        i = bisect_left(values, -const - a)
        if i:
            p_neg.append(pa * cum_p[i])
            e_neg.append(pa * ((h_full - const - a) * cum_p[i] - cum_v[i]))
    omitted = 1.0 - math.fsum(p for _, p in left) * cum_p[-1]
    worst = h_full - const - math.fsum(min(0.0, w) for w in weights[:-1])  # max of H - S
    return {
        "p": math.fsum(p_neg), "effect": math.fsum(e_neg), "omitted": max(omitted, 0.0), "error": max(omitted, 0.0) * worst,
        "mean": two_kmer_intercept(n_obs, minor, threshold, floor), "H": h_full,
    }


_CDF: dict[tuple[int, int, int], tuple[int, list[float]]] = {}


def draw_binomial(rng: random.Random, n: int, a: int, b: int) -> int:
    """One draw of Binomial(n, a / (a + b)) by inverting a cumulative table (one random())."""
    key = (n, a, b)
    table = _CDF.get(key)
    if table is None:
        lo, P = binomial_window(n, a, b)
        table = (lo, list(accumulate(P)))
        _CDF[key] = table
    lo, cum = table
    i = bisect_right(cum, rng.random() * cum[-1])
    return lo + min(i, len(cum) - 1)


class RunPosition:
    """A position prepared for simulating runs of v5.0.9's procedure.

    A subsample of n draws is drawn as: the number t of draws that are not the first (most
    common) k-mer ~ Binomial(n, (N - c_1) / N), then each of those t draws picks one of the
    other N - c_1 sequences uniformly (one random() each) -- the same distribution as n
    independent draws from all N sequences.
    """

    def __init__(self, counts: list[int], threshold: int, floor: bool = True):
        self.n_obs = sum(counts)
        self.h_full = entropy(counts)
        d = design(self.n_obs, threshold, floor)
        self.active = d is not None and len(counts) > 1
        self.minor_total = self.n_obs - counts[0]
        self.owner: list[int] = []
        for v, c in enumerate(counts[1:]):
            self.owner += [v] * c
        self.n_minor = len(counts) - 1
        if self.active:
            self.sizes, weights = d
            self.weights = weights[:-1]
            self.const = weights[-1] * self.h_full

    def intercept(self, rng: random.Random) -> float:
        parts = [self.const]
        rnd = rng.random
        for n, w in zip(self.sizes, self.weights, strict=True):
            t = draw_binomial(rng, n, self.minor_total, self.n_obs - self.minor_total)
            if t == 0:
                continue
            h = h_table(n)
            if self.n_minor == 1:
                y = h[t] + h[n - t]
            else:
                seen = [0] * self.n_minor
                for _ in range(t):
                    seen[self.owner[int(rnd() * self.minor_total)]] += 1
                y = h[n - t] + math.fsum(h[x] for x in seen if x)
            parts.append(w * y)
        return math.fsum(parts)


def simulate_rule(positions: list[RunPosition], runs: int, seed: int) -> list[float]:
    """Per run: the summed effect of the rule, sum over positions of (H - S) 1{S < 0}."""
    rng = random.Random(seed)
    active = [p for p in positions if p.active]
    out = []
    for _ in range(runs):
        effect = []
        for p in active:
            s = p.intercept(rng)
            if s < 0:
                effect.append(p.h_full - s)
        out.append(math.fsum(effect))
    return out


def mers_rule_task(counts: list[list[int]], threshold: int, runs: int, seed: int) -> list[float]:
    return simulate_rule([RunPosition(cs, threshold) for cs in counts], runs, seed)


def expected_means_task(counts: list[list[int]], threshold: int) -> tuple[float, float]:
    """Mean over the given (eligible) positions of DiMA's entropy (the reference
    implementation's dima_entropy) and of v5.0.9's expected intercept before the rule."""
    dima, v509 = [], []
    for cs in counts:
        mult = R.multiplicities(cs)
        n_obs = sum(cs)
        h_full = entropy(cs)
        dima.append(R.dima_entropy(mult, n_obs, threshold, h_full)[0])
        d = design(n_obs, threshold, True)
        if d is None or len(cs) == 1:
            v509.append(h_full)
            continue
        sizes, weights = d
        v509.append(math.fsum([weights[-1] * h_full] + [w * R.expected_plugin_entropy(mult, n_obs, n) for n, w in zip(sizes, weights[:-1], strict=True)]))
    return math.fsum(dima), math.fsum(v509)


def fallback_mc_task(n_obs: int, n_minor: int, runs: int, seed: int) -> dict:
    """Seeded Monte Carlo of one run at a position whose N - m sequences carry the Index and m
    sequences carry m different k-mers: fallback rate and the rule's mean effect."""
    counts = [n_obs - n_minor] + [1] * n_minor
    pos = RunPosition(counts, 100)
    rng = random.Random(seed)
    falls, effect = 0, []
    for _ in range(runs):
        s = pos.intercept(rng)
        if s < 0:
            falls += 1
            effect.append(pos.h_full - s)
        else:
            effect.append(0.0)
    mean = math.fsum(effect) / runs
    sd = math.sqrt(math.fsum((e - mean) ** 2 for e in effect) / (runs - 1))
    expected = R.dima_entropy(R.multiplicities(counts), n_obs, 100, entropy(counts))[0]
    return {"N": n_obs, "m": n_minor, "p": falls / runs, "p_se": math.sqrt(falls / runs * (1 - falls / runs) / runs),
            "effect": mean, "effect_se": sd / math.sqrt(runs), "mean": expected}


def fallback_exact_task(n_obs: int, minor: int, threshold: int) -> dict:
    out = rule_effect_exact(n_obs, minor, threshold, True)
    out["dima"] = two_kmer_intercept(n_obs, minor, threshold, False)
    out.update(N=n_obs, a=minor, T=threshold)
    # The printed rates have 0.1 percentage point and the raises 0.1% of resolution.
    if out["omitted"] > 1e-6 or out["error"] > 1e-4 * out["effect"]:
        raise RuntimeError(f"rule_effect_exact: too much omitted mass at N = {n_obs}, a = {minor}")
    return out


def check_rule_mc() -> None:
    """The Monte Carlo sampler against the exact two-k-mer results (within 4.5 standard errors),
    and rule_effect_exact against enumerating every outcome of a small case."""
    for n_obs, minor, threshold in ((200, 2, 100), (171, 1, 100)):
        exact = rule_effect_exact(n_obs, minor, threshold, True)
        pos = RunPosition([n_obs - minor, minor], threshold)
        rng = random.Random(7)
        runs = 20_000
        vals = []
        for _ in range(runs):
            s = pos.intercept(rng)
            vals.append(pos.h_full - s if s < 0 else 0.0)
        falls = sum(1 for v in vals if v > 0)
        mean = math.fsum(vals) / runs
        sd = math.sqrt(math.fsum((v - mean) ** 2 for v in vals) / (runs - 1))
        p_se = math.sqrt(exact["p"] * (1 - exact["p"]) / runs)
        if abs(falls / runs - exact["p"]) > 4.5 * p_se or abs(mean - exact["effect"]) > 4.5 * sd / math.sqrt(runs):
            raise RuntimeError(f"Monte Carlo sampler disagrees with the exact result at N = {n_obs}, a = {minor}")
    n_obs, minor, threshold = 10, 1, 8  # four subsample points (sizes 8, 8, 9, 9) and the 100% point
    sizes, weights = design(n_obs, threshold, True)
    h_full = h2_table(n_obs)[minor]
    atoms = []
    for n in sizes:
        pmf = [comb(n, x) * minor ** x * (n_obs - minor) ** (n - x) / n_obs ** n for x in range(n + 1)]
        atoms.append([(h2_table(n)[x], p) for x, p in enumerate(pmf)])
    p_neg, e_neg = [], []
    for combo in product(*[range(len(a)) for a in atoms]):
        p = math.prod(atoms[j][x][1] for j, x in enumerate(combo))
        s = weights[-1] * h_full + math.fsum(weights[j] * atoms[j][x][0] for j, x in enumerate(combo))
        if s < 0:
            p_neg.append(p)
            e_neg.append(p * (h_full - s))
    exact = rule_effect_exact(n_obs, minor, threshold, True, cut=1e-30)
    if abs(math.fsum(p_neg) - exact["p"]) > 1e-12 or abs(math.fsum(e_neg) - exact["effect"]) > 1e-12:
        raise RuntimeError("rule_effect_exact disagrees with enumeration")


# ---------------------------------------------------------------------------------------------
# Population study
# ---------------------------------------------------------------------------------------------


def population_task(n_obs: int, threshold: int, runs: int, seed: int) -> list[dict]:
    """The cells of one N and T: errors of DiMA's expectation, Miller-Madow, the observed
    entropy (exact) and v5.0.9's per-run procedure (exact expectation + Monte Carlo effect).

    A sample with no minor-variant sequence (M = 0) has entropy 0 and no effect, so the runs
    draw M from its distribution given M >= 1 and the mean effect is scaled by P(M >= 1)
    (stratified sampling; unbiased, and no run is spent on samples without the variant)."""
    rng = random.Random(seed)
    d509 = design(n_obs, threshold, True)
    if d509 is None:
        raise RuntimeError(f"population study: no regression at N = {n_obs}, T = {threshold}")
    sizes, weights = d509
    hs = h2_table(n_obs)
    out = []
    for a, b in POP_PHI:
        phi = a / b
        h_true = -(phi * math.log(phi) + (1 - phi) * math.log1p(-phi)) / LN2
        lo, P = binomial_window(n_obs, a, b - a)
        dima, mm, obs, v509 = [], [], [], []
        for i, p in enumerate(P):
            m = lo + i
            if 0 < m < n_obs:
                dima.append(p * two_kmer_intercept(n_obs, m, threshold, False))
                v509.append(p * two_kmer_intercept(n_obs, m, threshold, True))
            obs.append(p * hs[m])
            mm.append(p * (hs[m] + (1.0 / (2 * n_obs * LN2) if 0 < m < n_obs else 0.0)))
        # Runs given M >= 1 (the window never reaches M = N for these frequencies).
        start = 1 - lo if lo < 1 else 0
        p_some = math.fsum(P[start:])
        cum = list(accumulate(P[start:]))
        effects = []
        for _ in range(runs):
            m = lo + start + min(bisect_right(cum, rng.random() * cum[-1]), len(cum) - 1)
            parts = [weights[-1] * hs[m]]
            for n, w in zip(sizes, weights[:-1], strict=True):
                parts.append(w * h2_table(n)[draw_binomial(rng, n, m, n_obs - m)])
            s = math.fsum(parts)
            effects.append(hs[m] - s if s < 0 else 0.0)
        mean_effect = math.fsum(effects) / runs
        sd = math.sqrt(math.fsum((e - mean_effect) ** 2 for e in effects) / (runs - 1))
        out.append({
            "N": n_obs, "T": threshold, "phi": Fraction(a, b), "expected_minor": Fraction(n_obs * a, b),
            "dima": math.fsum(dima) / h_true - 1, "mm": math.fsum(mm) / h_true - 1, "observed": math.fsum(obs) / h_true - 1,
            "v509": (math.fsum(v509) + p_some * mean_effect) / h_true - 1,
            "v509_se": p_some * sd / math.sqrt(runs) / h_true,
        })
    return out


# ---------------------------------------------------------------------------------------------
# Tasks
# ---------------------------------------------------------------------------------------------


def run_task(name: str, args: tuple):
    """Entry point of the worker processes (a fixed list of deterministic tasks)."""
    return TASKS[name](*args)


TASKS = {
    "table1": table1,
    "table2": table2,
    "tobrfv": tobrfv,
    "procedure": random_procedure,
    "mers_rule": mers_rule_task,
    "expected_means": expected_means_task,
    "fallback_exact": fallback_exact_task,
    "fallback_mc": fallback_mc_task,
    "population": population_task,
}


def self_test() -> None:
    """Checks of the methods against enumeration and of the v5.0.9 reading (a few seconds)."""
    if v509_start(100, 55) != 56:
        raise RuntimeError("v5.0.9's grid start no longer reproduces the documented T = 55, N = 100 case")
    check_averages()
    check_moments()
    check_rule_mc()


def compute(workers: int) -> dict:
    """Runs every task (in a pool of ``workers`` processes) and collects the results."""
    self_test()
    for n_obs in FALLBACK_N + FEW_POINTS_N + POP_N:
        for threshold in POP_T:
            check_v509_start(n_obs, threshold)
    with concurrent.futures.ProcessPoolExecutor(max_workers=workers) as pool:
        jobs: dict[tuple, concurrent.futures.Future] = {}

        def submit(key, name, *args):
            jobs[key] = pool.submit(run_task, name, args)

        # Tasks that need no dataset first, so they run while the datasets are read.
        for n_obs in FALLBACK_N:
            for minor in FALLBACK_MINOR:
                submit(("fallback_exact", n_obs, minor), "fallback_exact", n_obs, minor, 100)
            for i, n_minor in enumerate(FALLBACK_MINOR[1:]):
                seed = MC_FALLBACK[0] * 1000 + FALLBACK_N.index(n_obs) * 10 + i
                submit(("fallback_mc", n_obs, n_minor), "fallback_mc", n_obs, n_minor, MC_FALLBACK[1], seed)
        for n_obs in FEW_POINTS_N:
            submit(("fallback_exact", n_obs, 1), "fallback_exact", n_obs, 1, 100)
        for ti, threshold in enumerate(POP_T):
            for ni, n_obs in enumerate(POP_N):
                seed = MC_POPULATION[0] * 1000 + ti * 100 + ni
                submit(("population", threshold, n_obs), "population", n_obs, threshold, MC_POPULATION[1], seed)
        for name in ("table1", "table2", "tobrfv"):
            submit((name,), name)
        counts = {}
        for dataset in ("mers_aa", "mers_nt", "hiv_dna", "hiv_pro"):
            _, positions = read_alignment(DATASETS[dataset])
            counts[dataset] = [[c for _, c in v] for v in positions]
            for cs in counts[dataset]:
                n_obs = sum(cs)
                if n_obs > 30:
                    for threshold in POP_T:
                        if n_obs > threshold:
                            check_v509_start(n_obs, threshold)
        for (dataset, threshold), (seed, runs, tasks) in MC_MERS.items():
            eligible = [cs for cs in counts[dataset] if sum(cs) > threshold]
            for i in range(tasks):
                submit(("mers_rule", dataset, threshold, i), "mers_rule", eligible, threshold, runs, seed * 1000 + i)
                submit(("expected_means", dataset, threshold, i), "expected_means", eligible[i::tasks], threshold)
        for dataset in ("hiv_dna", "hiv_pro"):
            numbers = [p + 1 for p, cs in enumerate(counts[dataset]) if sum(cs) > 100]
            chunks = 16 if dataset == "hiv_dna" else 4
            for i in range(chunks):
                sel = numbers[i::chunks]
                submit(("procedure", dataset, i), "procedure", sel, [counts[dataset][p - 1] for p in sel], 100)
        mers25 = counts["mers_aa"][24]
        submit(("procedure", "mers_aa_t30"), "procedure", [25], [mers25], 30)
        submit(("procedure", "mers_aa_t100"), "procedure", [25], [mers25], 100)
        results = {key: job.result() for key, job in jobs.items()}
    results["eligible"] = {
        (dataset, threshold): sum(1 for cs in counts[dataset] if sum(cs) > threshold)
        for dataset, threshold in list(MC_MERS) + [("hiv_dna", 100), ("hiv_pro", 100)]
    }
    return results


# ---------------------------------------------------------------------------------------------
# Summaries of the results
# ---------------------------------------------------------------------------------------------


def summarise(res: dict) -> dict:
    s: dict = {}
    # The random procedure at MERS-CoV position 25 and on HIV-1 pol.
    for threshold in (30, 100):
        (pos,) = res[("procedure", f"mers_aa_t{threshold}")]
        s[f"fig1c_{threshold}"] = {"mean": pos["ceil"]["mean"], "sd": math.sqrt(pos["ceil"]["var"])}
    for dataset, chunks in (("hiv_dna", 16), ("hiv_pro", 4)):
        rows = [r for i in range(chunks) for r in res[("procedure", dataset, i)]]
        rows.sort(key=lambda r: r["position"])
        m = len(rows)
        if m != res["eligible"][(dataset, 100)]:
            raise RuntimeError(f"{dataset}: eligible positions miscounted")
        s[dataset] = {
            "mean": math.fsum(r["ceil"]["mean"] for r in rows) / m,
            "sd_run_mean": math.sqrt(math.fsum(r["ceil"]["var"] for r in rows)) / m,
            "floor_shift": math.fsum(r["floor"]["mean"] - r["ceil"]["mean"] for r in rows) / m,
            "rule_bound": math.fsum(r["bound"] for r in rows) / m,
            "positions": {r["position"]: r for r in rows},
        }
    # v5.0.9's averages on MERS-CoV.
    for (dataset, threshold), (_, _, tasks) in MC_MERS.items():
        per_run = [x for i in range(tasks) for x in res[("mers_rule", dataset, threshold, i)]]
        m = res["eligible"][(dataset, threshold)]
        per_run = [x / m for x in per_run]
        mean = math.fsum(per_run) / len(per_run)
        sd = math.sqrt(math.fsum((x - mean) ** 2 for x in per_run) / (len(per_run) - 1))
        sums = [res[("expected_means", dataset, threshold, i)] for i in range(tasks)]
        dima = math.fsum(a for a, _ in sums) / m
        v509_expected = math.fsum(b for _, b in sums) / m
        s[f"v509_{dataset}_{threshold}"] = {"runs": len(per_run), "value": v509_expected + mean, "half_width": 1.96 * sd / math.sqrt(len(per_run)),
                                             "dima": dima}
    # Fallback rates.
    s["fallback_exact"] = {(n, a): res[("fallback_exact", n, a)] for n in FALLBACK_N for a in FALLBACK_MINOR}
    s["fallback_mc"] = {(n, m): res[("fallback_mc", n, m)] for n in FALLBACK_N for m in FALLBACK_MINOR[1:]}
    s["few_points"] = {n: res[("fallback_exact", n, 1)] for n in FEW_POINTS_N}
    s["population"] = [cell for threshold in POP_T for n in POP_N for cell in res[("population", threshold, n)]]
    s["table1"] = res[("table1",)]
    s["table2"] = res[("table2",)]
    s["tobrfv"] = res[("tobrfv",)]
    return s


# ---------------------------------------------------------------------------------------------
# Rendering
# ---------------------------------------------------------------------------------------------

DASH = "–"


def f4(x: float) -> str:
    return f"{x:.4f}"


def pct(x: float, digits: int = 1) -> str:
    return f"{100 * x:.{digits}f}%"


def span(values: list[float], fmt) -> str:
    """'lo–hi' of the formatted extremes ('20.2–21.6%' for percentages)."""
    lo, hi = fmt(min(values)), fmt(max(values))
    if lo == hi:
        return lo
    if lo.endswith("%") and hi.endswith("%"):
        lo = lo[:-1]
    return f"{lo}{DASH}{hi}"


def listing(items: list[str]) -> str:
    """'a, b and c'."""
    return items[0] if len(items) == 1 else ", ".join(items[:-1]) + " and " + items[-1]


def ceil_sig(x: float, digits: int = 2) -> str:
    """x rounded up to ``digits`` significant digits (for upper bounds)."""
    if x <= 0:
        return "0"
    exp = math.floor(math.log10(x)) - digits + 1
    scaled = math.ceil(round(x / 10 ** exp, 9))
    value = scaled * Fraction(10) ** exp
    decimals = max(0, -exp)
    return f"{float(value):.{decimals}f}"


def thousands(n: int) -> str:
    return f"{n:,}"


def half_width_text(hw: float, decimals: int) -> str:
    """The 95% half-width rounded up to the printed precision, checked to stay within it."""
    step = 10 ** -decimals
    units = math.ceil(hw / step - 1e-9)
    if units > 1:
        raise RuntimeError(f"Monte Carlo half-width {hw!r} exceeds the printed precision {step}: raise the run count")
    return f"±{step:.{decimals}f}"


def render_hiv_run_mean(s: dict) -> str:
    return (
        "and the mean intercept of one run of the random procedure, before its negative-intercept rule, "
        f"has a standard deviation of {f4(s['hiv_dna']['sd_run_mean'])} bits around it"
    )


def render_fig1c(s: dict) -> str:
    a, b = s["fig1c_30"], s["fig1c_100"]
    z = (a["mean"] - PAPER_FIG1C) / a["sd"]
    if not 0 < z < 1:
        raise RuntimeError("Fig. 1C: 1.38 is no longer an ordinary draw below the expectation")
    return (
        f"The exact expectation is {f4(a['mean'])} bits, and the intercept of one run of the random procedure "
        f"has a standard deviation of {f4(a['sd'])} bits there, so 1.38 lies {z:.2f} standard deviations below "
        f"the expectation: an ordinary draw of the random procedure, not a different method. With $T = 100$ "
        f"the expectation is {f4(b['mean'])} bits and the standard deviation {f4(b['sd'])} bits."
    )


def render_fallback_table(s: dict) -> str:
    exact, mc = s["fallback_exact"], s["fallback_mc"]
    lines = [
        "| Sequences that differ from the Index | Runs that fall back | Average raised by | DiMA's value |",
        "|---|---|---|---|",
    ]
    hw_p, hw_e = 0.0, 0.0
    names = {1: "one", 2: "two", 5: "five"}
    for m in FALLBACK_MINOR:
        cells = [exact[(n, m)] for n in FALLBACK_N]
        rates = [c["p"] for c in cells]
        raises = [c["effect"] / c["dima"] for c in cells]
        values = [c["dima"] for c in cells]
        label = names[m] if m == 1 else f"{names[m]}, {'both' if m == 2 else 'all'} carrying the same k-mer"
        lines.append(f"| {label} | {span(rates, pct)} | {span(raises, pct)} | {span(values, f4)} bits |")
        if m > 1:
            cells = [mc[(n, m)] for n in FALLBACK_N]
            rates = [c["p"] for c in cells]
            raises = [c["effect"] / c["mean"] for c in cells]
            values = [c["mean"] for c in cells]
            hw_p = max(hw_p, max(1.96 * c["p_se"] for c in cells))
            hw_e = max(hw_e, max(1.96 * c["effect_se"] / c["mean"] for c in cells))
            label = f"{names[m]}, {'carrying two different' if m == 2 else 'all carrying different'} k-mers"
            lines.append(f"| {label} | {span(rates, pct)} | {span(raises, pct)} | {span(values, f4)} bits |")
    hw = max(hw_p, hw_e)
    if hw > 0.005:
        raise RuntimeError(f"fallback Monte Carlo half-width {hw!r} above half a percentage point")
    note = (
        f"Each range is over $N$ = {listing([thousands(n) for n in FALLBACK_N])} sequences ($T = 100$), "
        "where the grid has all eleven points and the subsample sizes need no rounding, so v5.0.9's runs "
        "differ from the paper's procedure only by the rule. Exact where the differing sequences carry one "
        f"k-mer; seeded Monte Carlo of {thousands(MC_FALLBACK[1])} runs per $N$ where they carry different "
        f"ones (95% intervals within ±{ceil_sig(100 * hw, 1)} percentage points)."
    )
    return "\n".join(lines) + "\n\n" + note


def render_few_points(s: dict) -> str:
    parts = []
    for n in FEW_POINTS_N:
        c = s["few_points"][n]
        parts.append(f"by {100 * ((c['mean'] + c['effect']) / c['dima'] - 1):.0f}% at $N = {n}$")
    return f"one sequence differing raises the average {parts[0]} and {parts[1]} ($T = 100$; exact)"


def hiv_bounds(s: dict) -> tuple[str, str, str, str]:
    out = []
    for dataset in ("hiv_dna", "hiv_pro"):
        h = s[dataset]
        bound = h["rule_bound"] + abs(h["floor_shift"])
        if bound >= h["sd_run_mean"] / 2:
            raise RuntimeError(f"{dataset}: the bound is no longer under half a run's standard deviation")
        out.append((ceil_sig(bound, 2), f4(h["sd_run_mean"])))
    return out[0][0], out[1][0], out[0][1], out[1][1]


def render_v509_means(s: dict) -> str:
    aa = s["v509_mers_aa_100"]
    bd, bp, sdd, sdp = hiv_bounds(s)
    return (
        f"{f4(aa['value'])} bits averaged over v5.0.9 runs (seeded Monte Carlo of {thousands(aa['runs'])} runs, "
        f"95% interval {half_width_text(aa['half_width'], 4)}); on the HIV-1 *pol* datasets the two differ by less "
        f"than {bd} bits (DNA) and {bp} bits (protein), under half the standard deviation of the mean of a "
        f"single run ({sdd} and {sdp} bits)"
    )


def render_population(s: dict) -> str:
    cells = s["population"]
    below = [c for c in cells if c["expected_minor"] < 1]
    above = [c for c in cells if c["expected_minor"] >= 1]
    under_below = -min(c["dima"] for c in below)
    under_above = -min(c["dima"] for c in above)
    over_dima = max(c["dima"] for c in cells)
    over = {t: max(c["v509"] for c in cells if c["T"] == t) for t in POP_T}
    gap = max(abs(c["dima"] - c["mm"]) for c in cells)
    hw = max(1.96 * c["v509_se"] for c in cells)
    # "Neither had the smaller error everywhere", where the Monte Carlo decides the comparison.
    rule_better = any(abs(c["v509"]) + 1.96 * c["v509_se"] < abs(c["dima"]) for c in cells)
    dima_better = any(abs(c["dima"]) < abs(c["v509"]) - 1.96 * c["v509_se"] for c in cells)
    if not (rule_better and dima_better):
        raise RuntimeError("population study: one of the two now has the smaller error everywhere; re-word")
    if not 0 < over_dima < 0.005 or min(over.values()) <= 0:
        raise RuntimeError("population study: the signs of the errors changed; re-word")
    phis = listing([f"{float(100 * Fraction(a, b)):g}" for a, b in POP_PHI])
    return (
        f"In samples of $N$ = {listing([thousands(n) for n in POP_N])} sequences from two-variant "
        f"populations with a known entropy (minor-variant frequency {phis}%; $T$ = 30 and 100), v5.0.9's "
        f"per-run rule overestimated the entropy by up to {pct(over[100], 0)} on average with $T = 100$ and "
        f"{pct(over[30], 0)} with $T = 30$ (seeded Monte Carlo of {thousands(MC_POPULATION[1])} runs per case, "
        f"95% intervals within ±{ceil_sig(100 * hw, 1)} percentage points). The exact expectation "
        f"underestimated it by up to {pct(under_below, 0)} when less than one minor-variant sequence was "
        f"expected and by up to {pct(under_above)} otherwise (and overestimated it by at most "
        f"{ceil_sig(100 * over_dima, 2)}%), within {ceil_sig(100 * gap, 2)} percentage points of the error of the "
        "Miller–Madow correction. Neither v5.0.9's per-run rule nor the exact expectation had the smaller error "
        "everywhere."
    )


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


def share(p: float) -> str:
    """A probability as a percentage, '< 0.01%' for a positive one that would round to 0."""
    if p == 0:
        return "0%"
    if p < 0.00005:
        return "< 0.01%"
    return pct(p, 2)


def render_populations(s: dict) -> str:
    t = s["table2"]
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


def render_hiv_run_mean_note(s: dict) -> str:
    return (
        "the mean intercept of one run of the published procedure, before its negative-intercept rule, has "
        f"a standard deviation of {f4(s['hiv_dna']['sd_run_mean'])} bits (exact)"
    )


def render_hiv_positions(s: dict) -> str:
    pos = s["hiv_dna"]["positions"]
    a, b = pos[2176]["ceil"], pos[2177]["ceil"]
    sda, sdb = math.sqrt(a["var"]), math.sqrt(b["var"])
    if abs(a["mean"] - b["mean"]) >= min(sda, sdb):
        raise RuntimeError("positions 2176 and 2177 no longer differ by less than a standard deviation")
    sds = f"{f4(sda)} bits at both" if f4(sda) == f4(sdb) else f"{f4(sda)} and {f4(sdb)} bits"
    return (
        f"The expected corrected entropies at positions 2176 and 2177 are {f4(a['mean'])} and {f4(b['mean'])} bits, "
        f"a difference well within the spread of the random procedure at those positions (exact standard "
        f"deviation {sds})"
    )


def render_fig1c_note(s: dict) -> str:
    a, b = s["fig1c_30"], s["fig1c_100"]
    z = (a["mean"] - PAPER_FIG1C) / a["sd"]
    return (
        f"Its exact standard deviation at this position is {f4(a['sd'])} bits, and 1.38 lies {z:.2f} standard "
        f"deviations below the expectation, {f4(a['mean'])} ({f4(b['mean'])} with the default $T = 100$, "
        f"standard deviation {f4(b['sd'])})"
    )


def render_v509_comparison(s: dict) -> str:
    a100, a30, nt = s["v509_mers_aa_100"], s["v509_mers_aa_30"], s["v509_mers_nt_100"]
    hw = half_width_text(max(a100["half_width"], a30["half_width"], nt["half_width"]), 4)
    bd, bp, _, _ = hiv_bounds(s)
    return (
        f"the MERS-CoV spike protein means are {f4(a100['value'])} bits ($T = 100$) and {f4(a30['value'])} bits "
        f"($T = 30$) and the gene's {f4(nt['value'])} bits (seeded Monte Carlo of {thousands(a100['runs'])}, "
        f"{thousands(a30['runs'])} and {thousands(nt['runs'])} runs, 95% intervals {hw}), and the HIV-1 *pol* means differ "
        f"from DiMA's by less than {bd} bits (DNA) and {bp} bits (protein)"
    )


def render_validate(s: dict) -> str:
    pos = s["hiv_dna"]["positions"]
    sda, sdb = math.sqrt(pos[2176]["ceil"]["var"]), math.sqrt(pos[2177]["ceil"]["var"])
    sds = f"SD {f4(sda)} bits at positions 2176 and 2177" if f4(sda) == f4(sdb) else f"SD {f4(sda)} and {f4(sdb)} bits at positions 2176 and 2177"
    a = s["fig1c_30"]
    z = (a["mean"] - PAPER_FIG1C) / a["sd"]
    return "\n".join([
        f'RANGE_NOTE = "the extremes of one random run vary ({sds})"',
        f'FIG1C_NOTE = "1.38 is {z:.2f} SD below the exact expectation (SD {f4(a["sd"])} bits)"',
    ])


#: (file, region name, kind, renderer)
REGIONS = [
    (ENTROPY, "hiv-run-mean", "inline", render_hiv_run_mean),
    (ENTROPY, "fig1c", "inline", render_fig1c),
    (ENTROPY, "v509-fallbacks", "block", render_fallback_table),
    (ENTROPY, "v509-few-points", "inline", render_few_points),
    (ENTROPY, "v509-means", "inline", render_v509_means),
    (ENTROPY, "population-study", "block", render_population),
    (ENTROPY, "swe-subsets", "block", render_subsets),
    (ENTROPY, "swe-populations", "block", render_populations),
    (COMPARISON, "hiv-run-mean", "inline", render_hiv_run_mean_note),
    (COMPARISON, "hiv-2176-2177", "inline", render_hiv_positions),
    (COMPARISON, "fig1c", "inline", render_fig1c_note),
    (COMPARISON, "v509-means", "inline", render_v509_comparison),
    (COMPARISON, "tobrfv", "block", render_tobrfv),
    (VALIDATE, "notes", "code", render_validate),
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


def list_indent(line: str) -> str:
    m = re.match(r"(\s*)([-*+]|\d+[.)])(\s+)", line)
    if m:
        return " " * len(m.group(0))
    return re.match(r"\s*", line).group(0)


def apply_regions(text: str, path: Path, rendered: dict[str, str], kinds: dict[str, str]) -> str:
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
        start_line = text[line_start:i]
        before = start_line[: len(start_line) - len(start)]
        kind = kinds[name]
        if kind == "block":
            if before.strip():
                raise RuntimeError(f"{path.relative_to(ROOT)}: block region {name!r} must start on its own line")
            lines = []
            for line in content.split("\n"):
                if not line:
                    lines.append("")
                elif line.startswith("|"):  # table rows stay whole
                    lines.append(before + line)
                else:
                    lines.extend(wrap(line, before))
            body = "\n\n" + "\n".join(lines) + "\n\n" + before
        elif kind == "inline":
            if not before.strip() or not before.endswith(" "):
                raise RuntimeError(f"{path.relative_to(ROOT)}: inline region {name!r} must follow text and a space on its line")
            lines = wrap(content, list_indent(start_line))
            if _BLOCK_START.match(lines[0].lstrip()):
                raise RuntimeError(f"{path.relative_to(ROOT)}: inline region {name!r} would start a Markdown block")
            body = "\n" + "\n".join(lines)
        else:  # code
            if before.strip() != "#":
                raise RuntimeError(f"{path.relative_to(ROOT)}: code region {name!r} must be a comment line")
            indent = before[: before.index("#")]
            body = "\n" + "".join(indent + line + "\n" for line in content.split("\n")) + indent + "# "
        text = text[:i] + body + text[j:]
    return text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="fail if a region is out of date (prints the difference)")
    parser.add_argument("--workers", type=int, default=os.cpu_count() or 1, help="worker processes (default: all cores)")
    args = parser.parse_args()
    t0 = time.monotonic()
    summary = summarise(compute(max(1, args.workers)))
    by_file: dict[Path, dict[str, str]] = {}
    kinds: dict[Path, dict[str, str]] = {}
    for path, name, kind, render in REGIONS:
        by_file.setdefault(path, {})[name] = render(summary)
        kinds.setdefault(path, {})[name] = kind
    stale = []
    for path, rendered in by_file.items():
        current = path.read_text(encoding="utf-8")
        expected = apply_regions(current, path, rendered, kinds[path])
        if current == expected:
            continue
        rel = str(path.relative_to(ROOT))
        if args.check:
            sys.stdout.writelines(difflib.unified_diff(current.splitlines(True), expected.splitlines(True), rel, rel + " (expected)"))
            stale.append(rel)
        else:
            tmp = path.with_name(path.name + ".tmp")
            tmp.write_text(expected, encoding="utf-8", newline="\n")
            os.chmod(tmp, os.stat(path).st_mode)  # validate_paper.sh stays executable
            os.replace(tmp, path)
            print(f"wrote {rel}", file=sys.stderr)
    print(f"docs_numbers: {time.monotonic() - t0:.0f} s", file=sys.stderr)
    if stale:
        print(f"out of date: {', '.join(stale)}: run python3 scripts/reference/docs_numbers.py", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
