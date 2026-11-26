# DiMA reference implementation (test oracle)

`dima_reference.py` is an independent, standard-library-only Python implementation of
DiMA, the Rust re-implementation of

> Tharanga S, Ünlü ES, Hu Y, Sjaugi MF, Çelik MA, Hekimoğlu H, Miotto O, Öncel MM, Khan AM.
> DiMA: sequence diversity dynamics analyser for viruses. *Brief Bioinform*
> 2025;26(1):bbae607. doi:10.1093/bib/bbae607

It is **not** the product. It is a *test oracle*: slow, simple, obviously correct, and more
numerically accurate than the tolerance it is used with. Every rule is stated in the
docstring of the function that implements it.

## How the Rust tests use it

The Rust differential test (`crates/dima-core/tests/reference.rs`) runs the oracle and the
library on the same input with the same options. It compares every number within a
**relative 1e-13** (`|got − want| ≤ 1e-13·|want|`, so a zero must be exactly zero), and every
non-numeric field exactly: labels, k-mer sequences, motif classes, method and fallback names,
and region membership. The test alignments have at most a few thousand sequences per position;
the largest difference measured there is 2.6e-15, and a last-bit difference in a platform's
`log` moves the oracle by at most 6e-16 (see "Numerical accuracy" below for the limits at
larger N). The oracle's built-in checks run with the other pre-merge commands (see
`AGENTS.md`):

```bash
python3 scripts/reference/dima_reference.py --self-test
```

The paper test (`crates/dima-core/tests/paper.rs`) compares the library with this oracle's
output on the four bundled datasets, Wu 2025's two ToBRFV samples and the ten PSTVd loop-27
libraries of Wu 2025's Fig. 5c without running Python:
`paper_values.py` (in this directory) runs the oracle and writes those numbers to
`crates/dima-core/tests/paper_expected/mod.rs`, and CI fails when they are out of date:

```bash
python3 scripts/reference/paper_values.py           # after any change to the oracle's output
python3 scripts/reference/paper_values.py --check   # CI; about a minute on four cores
```

## Running it

Requires Python ≥ 3.10 and nothing else (no third-party packages). It has been checked
on CPython 3.10, 3.11, 3.12 and 3.13, which produce byte-identical output.

```bash
python3 scripts/reference/dima_reference.py analyze -i FILE [-k K] [-s T] [-a auto|protein|dna|rna]
    [--validation strict|permissive|report] [--header-format A|B|C] [--metadata-fields A|B]
    [--header-fillna Unknown] [--hcs-min-incidence 100] [--hcs-eligible-only]
    [--similarity-weighted-entropy] [-o out.json]
python3 scripts/reference/dima_reference.py --self-test
```

* `-i -` reads standard input. Plain, gzip and bzip2 input is accepted, detected by magic
  bytes, with every concatenated member and zero padding after the last one, as in DiMA; xz is
  refused, as by DiMA (Zstandard, which DiMA also reads, is not in the standard library).
* `-o` writes atomically (temporary file in the same directory, then rename). Without
  `-o`, JSON goes to stdout. Messages and progress go to stderr.
* Exit codes, as in DiMA: 0 success; 1 internal error or self-test failure; 2 usage error
  (including `k` outside `1..L`); 3 I/O error (a file that cannot be read); 4 invalid input
  data (including corrupt, truncated or trailing data in a compressed stream).

Measured on a 4-core Linux container with CPython 3.13, single-threaded:

| Dataset | Sequences × columns | k | Wall time | With `--similarity-weighted-entropy` |
|---|---|---|---|---|
| `samples/mers_spike_aa.fasta` | 191 × 1,353 | 9 | 0.3 s | 0.3 s |
| `tests/data/MERS_Spike_nt.fasta.bz2` | 499 × 4,065 | 27 | 2.4 s | 2.4 s |
| `tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2` | 3,756 × 1,197 | 9 | 11 s | 12 s |
| `tests/data/HIV1_FLT_2022_pol_DNA.fasta.bz2` | 2,637 × 3,390 | 27 | 19 s | 62 s |

Almost all of the time goes into the exact binomial expectations of the DiMA estimator
(see below); with the flag on HIV-1 pol DNA, about two thirds goes into its 2.0×10⁸ pairwise
comparisons of 27-mers. The self-test takes under a second.

## Specification summary

### Input and parsing

* Compression is detected by magic bytes: gzip `1F 8B`, bzip2 `BZh`. Concatenated members or
  streams are all decoded. A truncated stream or trailing non-stream data is an error. xz
  (`FD 37 7A 58 5A 00`) is refused with exit status 4.
* A UTF-8 BOM at the start is skipped. Lines are split on LF. A line starting with `>`
  begins a record: the header is the rest of the line, minus one trailing CR. Other lines
  are sequence lines.
* Sequence lines have space, TAB, CR, VT and FF removed wherever they occur, and ASCII
  lowercase folded to uppercase. A NUL byte is an error. Data before the first `>` is an
  error (an indented `>` too); lines there that are blank or hold only space, TAB, CR, VT
  and FF are ignored. Empty input, or input with no record, is an error.
* All records must have the same number of columns `L`, and `1 ≤ k ≤ L`.

### Alphabet

* `-a auto` (the default) ports Easel's `esl_abc_GuessAlphabet()` (EddyRivasLab/easel,
  `esl_alphabet.c`) branch by branch. It runs on the A–Z letter counts of DiMA's first
  batch: records up to 10,000, or up to 64 MiB once there are at least 1,000. If no branch decides, including when there are ≤ 10 residues,
  the run fails with an error unless `-a` is given.
* The output records which branch decided: `alphabet.rule` is one of `all_n`,
  `amino_only_letter`, `dna_composition`, `rna_composition` or `amino_composition`, and
  `null` when `-a` is explicit.
* Easel's last branch (`amino_composition`) cannot fire as written: it needs
  `x1+x2+x3+xn+xt ≥ 15`, but `x1 = 0` there, so the sum is at most 14. Protein is
  therefore only recognised through at least one of `E F I J L O P Q Z`. If both T and U
  exceed 2% of the letters, the result is undetermined.
* Default `k`: 9 for protein, 27 for DNA and RNA. Default `T`: 100.

### Symbol classes (after case folding)

| Alphabet | Residues (counted) | Excluded (k-mer not counted) |
|---|---|---|
| protein | `ACDEFGHIKLMNPQRSTVWY` `U` `O` | `X B J Z` and the common set |
| dna | `A C G T`, and `U` folded to `T` | `R Y K M S W B D H V N X I` and the common set |
| rna | `A C G U`, and `T` folded to `U` | `R Y K M S W B D H V N X I` and the common set |

The common excluded set is: gap `- . _`, missing `~ ?`, stop `* $ ^`, frameshift `#`.

Every other byte is *unknown*. In `strict` mode (the default) the first unknown byte is an
error that reports its record and 1-based column. In `permissive` and `report` modes,
unknown bytes are treated as excluded and counted on stderr. `report` also lists each
byte's count and first location, and each header with the wrong field count.

### Header metadata

Metadata is used only with `--header-format A|B|C`.

* The header is decoded as UTF-8. Invalid UTF-8 is an error in `strict` mode and replaced
  with U+FFFD otherwise. The header is then split on every `|`.
* If the field count is wrong, `strict` mode lists up to 20 offending records and fails.
  Other modes fill every field of that record with `--header-fillna` (default `Unknown`).
* Each value is cleaned in four steps:
  1. Delete general category Cf, and C0 (U+0000–U+001F) and C1 (U+0080–U+009F) controls,
     except TAB. DEL (U+007F) is in neither range and is kept.
  2. Strip leading and trailing Unicode White_Space, using an explicit set. Python's
     `str.strip()` is not the same.
  3. Normalise to NFC.
  4. If the result is empty, use the fill value.
* `--metadata-fields` selects a subset, kept in header-format order.
* Each motif's `metadata` is `{field: {value: count}}`, fields in format order and values
  sorted by code point. It counts the records that actually carry that k-mer at that
  position.

### Counting, labels, motifs

* For each 0-based start `p` in `0..L−k`, a record's k-mer is counted only if all `k`
  columns are residues. `support` is the number of counted k-mers.
* Labels: `NS` when support = 0; `LS` when 0 < support < T; `ELS` when support = T;
  `null` (eligible) when support > T.
* Motifs (DiMA v5.0.9 rules), with `max` the largest count:
  * Index: count = max and count > 1. Ties give several Index variants.
  * Unique: count = 1.
  * Of the rest, Major has the largest count among them (ties give several) and Minor is
    everything else.
* Variants are sorted by count descending, then k-mer ascending.
* Incidence is `100·count/support`.
* `distinct_variants_count` is the number of non-Index variants.
* `total_variants_incidence` is `100·Σ non-Index counts / support`, or `null` when
  support = 0.
* `distinct_variants_incidence` is `100·distinct_variants_count / Σ non-Index counts`, or
  `null` when that sum is 0.

### Entropy estimators (bits)

Notation: `c_i` are the counts, `f_c` the number of distinct k-mers with count `c`,
`N` the support, `D` the number of distinct k-mers.

* **plug-in** (`observed_entropy`): `−Σ (c/N) log2(c/N)`; `null` when N = 0.
* **dima** (`entropy`, the default) is the exact expectation of the paper's Fig. 2
  procedure:
  * N = 0 gives `null` with method `not_estimable` (`entropy_fallback` is `null`).
  * N ≤ T falls back to plug-in (`support_at_or_below_threshold`).
  * Otherwise compute `x = ⌈100T/N⌉` with integers. The start percentage is 50 if
    `x < 50`, else `x` rounded up to a multiple of 5. The percentages run from the start
    to 95 in steps of 5, giving sample sizes `n = ⌈R·N/100⌉`.
  * Each resampled point is `(1/n, E[H_n])`, where `E[H_n] = Σ_c f_c·E[h(X)]`,
    `X ~ Binomial(n, c/N)` and `h(x) = −(x/n) log2(x/n)`.
  * The 100% point is `(1/N, plug-in)`.
  * Fewer than 5 points falls back to plug-in (`too_few_points`).
  * Otherwise fit two-pass centred OLS of y on x, with every sum taken by `math.fsum`. If
    `sxx = 0`, fall back (`degenerate_regression`). Otherwise the intercept is the
    entropy, with method `dima`. The intercept cannot be negative for real count profiles
    (`E[H_n]` never falls as n grows and never exceeds the plug-in value, so it is at least
    the mean of the points); a negative one raises an internal error, as in DiMA.
* **miller_madow**: `H + (D−1)/(2N ln 2)`. N = 1 gives 0, N = 0 gives `null`.
* **chao_wang_jost**: Chao, Wang & Jost 2013, *Methods Ecol Evol* 4:1091,
  doi:10.1111/2041-210X.12108, converted from nats by ÷ ln 2. N = 1 gives 0,
  N = 0 gives `null`.
* **sample_coverage**: Chao & Jost 2012, *Ecology* 93:2533, doi:10.1890/11-1952.1, with
  the `f2 = 0` form from the specification. N < 2 gives `null`.
* **similarity_weighted_entropy** (with `--similarity-weighted-entropy`; otherwise `null`,
  like the two values below): Wu 2025, *Virus Evol* 11:veaf029, doi:10.1093/ve/veaf029,
  Section 2.1: `Σ_i (−f_i log2 f_i)(1 − S_i)` with `f_i = c_i/N`,
  `S_i = Σ_{j≠i} f_j s_ij` and `s_ij = m_ij/k`, where `m_ij` is the number of identical
  residues of k-mers `i` and `j`. Computed **pairwise**, as Wu defines it (every unordered
  pair once; `m_ij` from the zero bytes of the XOR of the k-mers' big-endian integers), with
  exact rational weights and 80-digit terms; never with the Rust library's per-site counts.
  0 with one distinct k-mer; `null` when N = 0.
* **normalised_observed_entropy**, **normalised_similarity_weighted_entropy**: the plug-in
  and similarity-weighted entropies divided by `log2 D` (D includes the Index), capped at 1;
  `null` when D ≤ 1.

### Summaries

* Top-level `average_entropy`, `lowest_entropy`, `highest_entropy` and
  `eligible_position_count` are computed over eligible positions (support > T).
* Each extreme is `{position, entropy, tied_position_count}`: the first 1-based position
  that attains it, plus the number of positions with exactly equal value.
* These are `null` when no position is eligible.
* `all_positions_summary` covers every position, counting NS as 0.0.
* `support_label_counts` gives the count for each label;
  `low_support_count = NS + LS + ELS`.
* `similarity_weighted_entropy_summary` (with the flag; otherwise `null`): `position_count`,
  `average_entropy`, `lowest_entropy`, `highest_entropy` of the similarity-weighted entropy
  and `average_observed_entropy` of the plug-in entropy, all over the eligible positions.

### HCS (historically conserved sequences)

**Candidates.** `--hcs-min-incidence P` must be a plain decimal with 50 < P ≤ 100 (default
100). A position is a candidate if an Index exists (count > 1) and
`count/support ≥ P/100`, compared as exact rationals. Any position may be a candidate;
with `--hcs-eligible-only`, only positions with support > T.

**Merging.** Candidates are taken in position order. A candidate joins the current region
only if all of these hold:

* it overlaps or abuts the last member (`p ≤ last + k`);
* its k-mer agrees with the region's sequence on every overlapping column;
* with `--hcs-eligible-only`, no position strictly between the last member and `p` has
  support ≤ T.

Otherwise it starts a new region. `split_from_previous` is true only when the new region
overlaps or abuts the previous one but the sequences disagree.

**Regions** report:

* `start_column`, `end_column` (inclusive, 1-based) and `length`;
* `sequence`, `min_incidence`, `min_support`;
* `members`, each with its 95% Clopper–Pearson lower bound (two-sided; `x = n` uses the
  closed form `0.025^(1/n)`, otherwise bisection on `[0, 1]` until the bracket is two
  adjacent doubles);
* `low_support_positions`: every position from the first to the last member with
  support ≤ T, whether or not it is a member;
* `flagged`, which is true when `low_support_positions` is non-empty;
* the minimum bound over the members.

### Output

Every float is finite, -0.0 is written as 0.0, and positions are 1-based. The output is
`json.dumps(result, sort_keys=False, indent=None, separators=(",", ":"))` followed by one
newline. Top-level key order:

`reference_version`, `sequence_count`, `alignment_length`, `kmer_length`,
`support_threshold`, `alphabet`, `support_label_counts`, `low_support_count`,
`average_entropy`, `lowest_entropy`, `highest_entropy`, `eligible_position_count`,
`all_positions_summary`, `similarity_weighted_entropy_summary`, `positions`, `hcs`.

Positions are written to a temporary file one at a time and spliced in, so large
alignments never hold all motif lists in memory. `--self-test` checks that the result is
byte-identical to a single `json.dumps` of the whole object. `hcs.threshold_percent` is
the canonical decimal text of P: no exponent, no leading zeros, no trailing fractional
zeros (`070.50` becomes `70.5`).

## Numerical accuracy

The oracle is designed to be far more accurate than the 1e-13 comparison tolerance and
than the Rust code itself:

* **Plug-in, Miller–Madow, Chao–Wang–Jost.** These are computed in 60–80-digit `decimal`
  arithmetic, with harmonic numbers in Decimal, and converted to binary64 once.
  * CWJ's second term is evaluated either in the paper's form (when `(1−A)^N ≥ 1e-15`)
    or as the equivalent positive series `(f1/N)(1−A) Σ_{j≥0} (1−A)^j/(N+j)`. That
    avoids the catastrophic cancellation the plain float formula suffers: up to ~7e-13
    relative error on the bundled datasets.
* **Sample coverage and incidences.** These are exact rationals, correctly rounded.
* **E[H_n].** This uses the binomial pmf, normalised at the mode, built with the exact
  ratio recurrence. With integer `c` and `N`, every ratio is one correctly rounded
  division of exact integers. The sum is `fsum(w·h)/fsum(w)`, which cancels any common
  factor exactly.
  * The walk keeps every term down to 2^-997 of the largest. By log-concavity the omitted
    remainder is below 2^-966 of the sum, so this is the full sum over `x = 0..n` for all
    practical purposes.
  * The textbook `exp(lgamma(n+1) − lgamma(x+1) − lgamma(n−x+1) + …)` is deliberately not
    used. For n ≈ 3,000, `lgamma(n+1) ≈ 2·10^4` carries several 1e-12 of absolute
    rounding error, which would put ~1e-11 relative error on every pmf.
  * `h(x)` uses `log1p((x−n)/n)` for `x ≥ n/2`.
* **Clopper–Pearson tail.** This uses the same normalised walk. `--self-test` checks the
  exact rational tail at the returned bound.

Verification performed while writing it:

* **Correct rounding.** Plug-in, Miller–Madow, CWJ and coverage were correctly rounded at
  every position of the MERS (protein and nucleotide) and HIV-1 protein datasets (5,902
  positions), against an independent 110-digit evaluation.
* **dima intercepts.** These agreed with an exact rational evaluation to ≤ 4.2e-16
  relative at all 2,345 non-trivial positions checked: every MERS protein position at
  T = 30 and T = 100, and every 7th MERS nucleotide position. HIV-1 DNA position 2176
  (N = 2,632, 1,325 distinct 27-mers) is correctly rounded.
* **Self-test.** `--self-test` compares `E[H_n]` with brute-force multinomial
  enumeration, exact rational pmfs and the lgamma form. It compares the two CWJ tail
  paths with each other (to 1e-40) and with the float formula on small N, and checks the
  Clopper–Pearson bounds against exact rational tails.

Limits of the comparison at large N:

* **Rounding in the Rust code.** The Rust code computes `−p·log2 p` from a rounded
  `p = c/N` and the binomial odds from a rounded `p`. The oracle uses `log1p` and exact
  integer odds. At a highly conserved position (one dominant k-mer) the Rust values
  therefore differ from the oracle's by up to about 2e-12 relative at 2.2 million
  sequences and 5e-11 at 17.5 million. Chao–Wang–Jost agrees to a few units in the last
  place at any N, because the Rust code evaluates its second term exactly (a hypergeometric
  series or continued fraction) while the oracle sums it in 80-digit `Decimal`; the two
  share no derivation. Similarity-weighted entropy shares the entropy terms' rounding but
  is smaller, so its relative difference is larger: up to about 3.4e-11 at 2.2 million
  sequences and 5.3e-10 at 17.5 million (the longer the k-mers, the larger).
* **What the tests can detect.** A differential test at that scale could not separate a
  bug smaller than those numbers from rounding. No test runs at that scale. On the
  bundled datasets the Rust values are within about 5e-15 of the oracle's
  (Chao–Wang–Jost within 4.2e-16; similarity-weighted entropy within 2e-14, 6e-15 on the
  MERS-CoV spike protein).

The measurements, why the rounding never matters scientifically (it is at least 10^10 times
smaller than the sampling error, 1.9e9 times for similarity-weighted entropy), and why the prototyped fix is deferred are in the
documentation page "Numerical accuracy" (`docs/src/content/docs/explanation/numerical-accuracy.md`).
For one differing sequence at N = 2,208,211 (`N − 1` copies of `A` and one `C`, run with
`-a dna -k 1`), `observed_entropy` differs from the oracle by 2.2e-12 relative and `entropy` by
1.3e-12.

## Determinism

* No randomness, timestamps or absolute paths appear in the output. Every list in the
  output is in an explicit order (variants are sorted), and caches are keyed by exact
  integers. Repeated runs are byte-identical.
* Floats are printed with `repr` (shortest round-trip), and NaN or infinity raise an
  error.
* `decimal` (libmpdec), `fractions`, integer arithmetic and `math.fsum` are exact or
  correctly rounded on every platform. Plug-in, Miller–Madow, CWJ, similarity-weighted
  entropy and both normalised entropies, coverage, incidences, supports and all counts are
  therefore platform independent.
* The dima entropy (through `E[H_n]`) and the Clopper–Pearson bound also use
  `math.log`, `math.log1p` and `**`, which come from the platform C library. Their last
  bit may differ between operating systems; that is ~1e-16 and irrelevant at 1e-13.
* Metadata cleaning depends on the Unicode database of the running Python (Cf category
  and NFC): 13.0 on 3.10, 14.0 on 3.11, 15.0 on 3.12, 15.1 on 3.13. Only characters
  assigned in later Unicode versions can be affected.

## Relationship to DiMA v5.0.9

Checked against the fixtures in `tests/data/dima-v5.0.9/` (k = 9, T = 30):

* Supports, labels, distinct-variant counts, variant and total incidences, and motif
  classes all agree.
* v5.0.9 does not sort motifs.
* v5.0.9 prints 0.0 where this specification gives `null`.
* v5.0.9's entropy correction is randomised.
* v5.0.9 attributes metadata to the i-th header in file order for the i-th *counted*
  k-mer, so it misattributes metadata whenever an earlier record is skipped at that
  position (for example records with gaps). This affects 187 motif entries in
  `advance_output.json` and 2,391 in `incons_headers_output.json`. The oracle attributes
  metadata to the records that actually carry the k-mer.
