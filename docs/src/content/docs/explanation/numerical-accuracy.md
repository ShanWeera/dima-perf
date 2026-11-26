---
title: Numerical accuracy
description: How accurate DiMA's floating-point results are, measured up to the size of the largest viral alignments, why the remaining rounding error never matters scientifically, and why a prototyped fix for it is deferred.
---

DiMA computes in IEEE 754 double precision, where every operation rounds to about 16 significant
digits. This page measures how much that rounding adds up to at the sizes DiMA is used for, and
compares it with the uncertainty that comes from sampling. Analysing round-off in this way, and
showing that it is tolerable, is what the field's software-quality literature asks of
scientific software [@darriba2018; @eddy2011].

## Summary

- **Always present, always negligible.** Every result carries some rounding error, as in any
  floating-point program. It grows with the number of sequences $N$ at a position, most at
  highly conserved positions (one dominant k-mer), which are most positions of a viral alignment.
- **Bundled datasets** (the paper's four datasets, up to 3,756 sequences): every entropy is
  within about $5 \times 10^{-15}$ of the exact result, relative to its size. That is the last one
  or two of the 17 significant digits written to the JSON. The optional similarity-weighted
  entropy is within $2 \times 10^{-14}$ (see
  [Similarity-weighted entropy](#similarity-weighted-entropy)).
- **The DiMA paper's SARS-CoV-2 scale** (about 2.2 million sequences per position): a default
  run (the `dima` estimate and the observed entropy) is within $2.2 \times 10^{-12}$. At
  17.5 million sequences it is within $5.1 \times 10^{-11}$ (from the 11th significant digit
  on). The opt-in Chao–Wang–Jost estimator, whose second term is evaluated exactly (see
  [Chao–Wang–Jost's second term](#chaowangjosts-second-term)), stays within a few units in the
  last place at any size. Similarity-weighted entropy, which is smaller than the observed entropy
  but has the same absolute error, is within $3.4 \times 10^{-11}$ (at 17.5 million sequences
  $5.3 \times 10^{-10}$), whatever the k-mer length. In absolute terms no error reaches $10^{-14}$ bits.
- **Far below the sampling error.** The rounding error is $10^{10}$ to $10^{12}$ times smaller
  than the standard deviation of the entropy estimate itself ($1.9 \times 10^9$ to
  $2.2 \times 10^{11}$ times for similarity-weighted entropy). The paper reports entropies to two
  decimals.
- **Results stay deterministic either way.** Identical inputs give byte-identical outputs on
  every platform and thread count (see
  [Numerical details](/explanation/entropy/#numerical-details)).
- **A fix for the remaining last digits was prototyped and measured, and is deferred.** It
  brings almost every value to within about $10^{-15}$, but it changes stored digits without
  changing any scientific result (see [Why the fix is deferred](#why-the-fix-is-deferred)).

## Where the rounding error comes from

Three computations lose accuracy as $N$ grows. Chao–Wang–Jost's second term would be a fourth
if it were summed as written; DiMA evaluates it exactly instead (see
[Chao–Wang–Jost's second term](#chaowangjosts-second-term)). Everything else is exact, rounded
once, or measured to be accurate:

- counts, supports and labels are integers;
- an incidence is a single division;
- the conserved-sequence threshold is compared in exact integer arithmetic;
- Clopper–Pearson bounds are within a few units in the last place (2 measured); most of that
  comes from $\alpha/2$ itself, which is computed as $(1 - 0.95)/2 = 0.025000000000000022$
  instead of $0.025$ (part of the deferred fix below).

| Computation | What happens | Affects |
|---|---|---|
| The entropy term $-p \log_2 p$ of a k-mer whose count $c$ is close to $N$ | $p = c/N$ is rounded before the logarithm is taken, and the logarithm of a number close to 1 loses the low-order digits of its distance from 1 [@goldberg1991, §1.4 and Theorem 4], which magnifies the rounding by about $N/(N-c)$ | observed (plug-in) entropy, Miller–Madow, the expected resampled entropies $E[H_n]$ and so the `dima` estimate |
| The binomial weights of $E[H_n]$ | the odds $p/(1-p)$ are formed from the rounded $p$, and $1 - p$ loses accuracy in the same way | the `dima` estimate |
| Sample coverage $1 - (f_1/N)\,A'$ | the subtraction cancels when nearly every k-mer is a singleton, so that coverage is close to 0 | `sample_coverage` |

## Measurements

### How large N gets

The DiMA paper downloaded "All 11,808,363 available Spike protein sequence records" of
SARS-CoV-2 (July 2022) and deduplicated them to 2,106,985 sequences. Its most diverse position
had a support of 2,208,211 sequences with 343 distinct 9-mers, 108 of them seen once
[@tharanga2025]. The measurements use that $N$, and 17.5 million sequences: about 1.5 times the
paper's raw records, to allow for growth and for analyses without deduplication.

### Method

Each value was compared with the same formula evaluated in 50-digit arithmetic (Python
`mpmath`; for Chao–Wang–Jost, the reference implementation's 80-digit decimal sum). The count
profiles model real positions:

- **one differing sequence:** a dominant k-mer and a single singleton, counts $(N-1, 1)$; the
  most common shape at a conserved position;
- **near-conserved:** a dominant k-mer, one doubleton and four singletons;
- **many doubletons:** a dominant k-mer, nine doubletons and two singletons, so that
  $AN \approx 9$, the hardest case for a term-by-term summation of Chao–Wang–Jost's second term;
- **diverse:** modelled on the paper's most diverse position, with 350 distinct k-mers, 100 of
  them singletons ($N = 2{,}213{,}789$).

### Relative error

The table gives $|\text{computed} - \text{exact}| / \text{exact}$:

| Profile | $N$ | Plug-in | Miller–Madow | `dima` | Chao–Wang–Jost | Coverage |
|---|---|---|---|---|---|---|
| one differing sequence | 2,208,211 | $2.2 \times 10^{-12}$ | $2.1 \times 10^{-12}$ | $1.3 \times 10^{-12}$ | $1.3 \times 10^{-16}$ | exact |
| near-conserved | 2,208,211 | $4.3 \times 10^{-13}$ | $4.1 \times 10^{-13}$ | $6.7 \times 10^{-13}$ | $3.7 \times 10^{-17}$ | $5.5 \times 10^{-17}$ |
| many doubletons | 2,208,211 | $1.6 \times 10^{-13}$ | $1.6 \times 10^{-13}$ | $2.5 \times 10^{-13}$ | $3.1 \times 10^{-17}$ | $8.4 \times 10^{-17}$ |
| diverse | 2,213,789 | $2.1 \times 10^{-17}$ | $1.0 \times 10^{-16}$ | $4.2 \times 10^{-15}$ | $4.7 \times 10^{-17}$ | $2.7 \times 10^{-17}$ |
| one differing sequence | 17,500,000 | $3.0 \times 10^{-11}$ | $2.9 \times 10^{-11}$ | $5.1 \times 10^{-11}$ | $2.3 \times 10^{-18}$ | exact |
| near-conserved | 17,500,000 | $7.0 \times 10^{-12}$ | $6.8 \times 10^{-12}$ | $6.7 \times 10^{-12}$ | $6.2 \times 10^{-17}$ | $4.1 \times 10^{-17}$ |
| many doubletons | 17,500,000 | $2.4 \times 10^{-12}$ | $2.3 \times 10^{-12}$ | $3.3 \times 10^{-12}$ | $1.8 \times 10^{-16}$ | $1.0 \times 10^{-17}$ |

The error grows roughly in proportion to $N$. For one differing sequence the plug-in error is
$2.0 \times 10^{-16}$ at 1,000 sequences, $3.6 \times 10^{-13}$ at 100,000 and
$3.1 \times 10^{-11}$ at 10 million. Positions where no k-mer dominates stay between
$10^{-15}$ and $10^{-14}$ even at 10 million sequences.

Chao–Wang–Jost stays within a few units in the last place at every size because its second
term is evaluated exactly. A term-by-term summation of that term loses up to 8 digits near
$AN = 9$: $4.6 \times 10^{-9}$ on a profile with 17.5 million sequences (one dominant k-mer,
1,000 singletons and 4,450 doubletons), the worst of 2,427 profiles searched. There the exact
value is $0.014173273093526109$ bits and DiMA writes `0.014173273093526107`.

### Compared with the sampling error

An entropy estimated from $N$ sequences has a standard deviation of about $\sigma/\sqrt{N}$, with
$\sigma^2 = \sum_i p_i(-\log_2 p_i - H)^2$; the plug-in and Miller–Madow estimators share this
rate [@paninski2003, Theorem 2]. The last column divides the largest absolute rounding error of
any estimator by that standard deviation:

| Profile | $N$ | Entropy (bits) | Standard deviation (bits) | Largest rounding error (bits) | Ratio |
|---|---|---|---|---|---|
| one differing sequence | 2,208,211 | $1.0 \times 10^{-5}$ | $9.5 \times 10^{-6}$ | $2.2 \times 10^{-17}$ | $2 \times 10^{-12}$ |
| near-conserved | 2,208,211 | $6.0 \times 10^{-5}$ | $2.3 \times 10^{-5}$ | $4.0 \times 10^{-17}$ | $2 \times 10^{-12}$ |
| many doubletons | 2,208,211 | $2.0 \times 10^{-4}$ | $4.1 \times 10^{-5}$ | $5.0 \times 10^{-17}$ | $1 \times 10^{-12}$ |
| diverse | 2,213,789 | 1.47 | $1.1 \times 10^{-3}$ | $6.2 \times 10^{-15}$ | $6 \times 10^{-12}$ |
| one differing sequence | 17,500,000 | $1.5 \times 10^{-6}$ | $1.4 \times 10^{-6}$ | $7.4 \times 10^{-17}$ | $5 \times 10^{-11}$ |
| near-conserved | 17,500,000 | $8.6 \times 10^{-6}$ | $3.3 \times 10^{-6}$ | $6.0 \times 10^{-17}$ | $2 \times 10^{-11}$ |
| many doubletons | 17,500,000 | $2.8 \times 10^{-5}$ | $5.9 \times 10^{-6}$ | $9.3 \times 10^{-17}$ | $2 \times 10^{-11}$ |

Drawing another sample of the same size would typically move each of these entropies
$10^{10}$ to $10^{12}$ times further than rounding does.

### Sample coverage in real data

The rounding error of sample coverage is about $10^{-16}/\hat C$, so it matters only when
coverage $\hat C$ is close to 0. That requires nearly every k-mer at a position to be a
singleton. An error of $10^{-9}$, for example, needs a coverage below $10^{-7}$, which at
10 million sequences means more than 99.99999% singletons. Real alignments are nowhere near this:

- In the four bundled datasets, positions where more than half of the sequences carry a k-mer
  seen only once occur only with 100 sequences or fewer. Coverage there is at least 0.07.
- Among positions with more than 100 sequences, the largest singleton share is 0.39, so coverage
  is at least 0.61.
- The paper's most diverse SARS-CoV-2 position has 108 singletons among 2,208,211 sequences, a
  share of $4.9 \times 10^{-5}$.
- The expected singleton share $\sum_i p_i(1 - p_i)^{N-1}$ falls as $N$ grows.

### The bundled datasets

At the sizes of the bundled datasets the current code is within about $5 \times 10^{-15}$ of
the exact values of every entropy (similarity-weighted entropy within $1.9 \times 10^{-14}$, see
[Similarity-weighted entropy](#similarity-weighted-entropy)), and Chao–Wang–Jost within $4.2 \times 10^{-16}$ (at most 3 units in the last
place).

One position of the small test alignment in `tests/golden/tiny/` splits seven sequences 4 to 3. Its exact plug-in
entropy is $0.985228136034251458\ldots$ bits. The nearest double is `0.9852281360342514`, and
DiMA writes `0.9852281360342515`, one unit in the last place above it.

### The one limit

With more than $2^{53} \approx 9 \times 10^{15}$ sequences at a position, a ratio such as $c/N$
can round to exactly 1, and an expected entropy can become undefined. DiMA checks that every
reported entropy is a finite number of at least 0 bits (see
[Design rationale](/explanation/design-rationale/#engine-and-performance)). Such a run
therefore stops with an internal error (exit 1) instead of writing a wrong value. That limit is about 760 million times
the 11.8 million spike records of 2022.

### Reproducing a measurement

The reference implementation in `scripts/reference/` (see
[Testing](/contributing/testing/)) computes the affected terms more accurately: in 60–80-digit
decimal arithmetic, with exact integer odds and `log1p`. Comparing the two on an alignment
with one of the profiles above reproduces the table. For one differing sequence at
$N = 2{,}208{,}211$:

```sh
python3 -c "import sys; n = 2208211; sys.stdout.write('>s\nA\n' * (n - 1) + '>s\nC\n')" > one.fa
dima analyze -i one.fa -a dna -k 1 -o dima.json
python3 scripts/reference/dima_reference.py analyze -i one.fa -a dna -k 1 -o ref.json
```

`observed_entropy` (`results[0]` against `positions[0]`) differs by $2.2 \times 10^{-12}$
relative, and `entropy` by $1.3 \times 10^{-12}$. Adding `--estimator miller-madow` or
`--estimator chao-wang-jost` to the `dima` run gives the Miller–Madow and Chao–Wang–Jost
values, which the reference reports as `miller_madow` and `chao_wang_jost`.

## Chao–Wang–Jost's second term

The second term of the Chao–Wang–Jost estimator [@chao2013] is
$\frac{f_1}{N}(1-A)^{1-N}\bigl(-\ln A - \sum_{r=1}^{N-1}(1-A)^r/r\bigr)$. Evaluated as written
in double precision it can fail completely: $(1-A)^{1-N}$ reaches about $10^{17}$ and multiplies
a difference of nearly equal numbers. For 1,000 sequences with one singleton and 20 doubletons
it gives 0.62 nats instead of $2.4 \times 10^{-5}$ (measured). Summing it term by term avoids
that, but costs time proportional to $N$ at every position with singletons (6.4 ms per position
at 2.2 million sequences, about 10% of a whole run at any size) and still loses up to 8 digits
near $AN = 9$.

DiMA evaluates the same quantity exactly, in a time independent of $N$. Because
$-\ln A = \sum_{r \ge 1}(1-A)^r/r$, the term equals $\frac{f_1}{N}(1-A)\,S$ with
$S = \sum_{j \ge 0}(1-A)^j/(N+j) = {}_2F_1(1, N; N+1; 1-A)/N$, which follows from the series
of the hypergeometric function [@hankin2015, eq. 15.1.1]. With $x = NA$:

- **Series** (for $N \ge 64$ and $x$ below about 0.56): the expansion of
  ${}_2F_1(a, b; a+b; z)$ about $z = 1$ [@hankin2015, eq. 15.3.10] gives $S$ as a sum of
  non-negative terms that shrink at least as fast as $x^k$; at most 17 terms are needed. Its
  digamma values come from an asymptotic series whose remainder is smaller than the first
  omitted term [@alzer1997, Theorem 8].
- **Continued fraction** (otherwise): Pfaff's transformation [@hankin2015, eq. 15.3.4] and
  Gauss's continued fraction, which converges to the ratio of hypergeometric functions
  [@vanvleck1901, p. 1 and §9(a)], give a continued fraction with positive coefficients at a
  positive argument. Its value lies between any two consecutive approximants
  [@henrici1966, §2], and evaluating an approximant backwards keeps its rounding error bounded
  [@jones1974, Theorem 5.1(i)]. At most 817 steps are needed below 64 sequences (every possible
  count profile was checked) and at most 373 above (measured).

Both stop once the remaining truncation error is below half a unit in the last place. A cap of
2048 steps turns any unforeseen failure to converge into an internal error, never a wrong value
or a hang.

Measured against the exact values, every position of a synthetic spike alignment of 2.2 million
sequences is within $3.7 \times 10^{-16}$ (at most 3 units in the last place), and every position
of the bundled datasets within $4.2 \times 10^{-16}$. The term no longer costs anything noticeable:
on that alignment the statistics phase went from 7.97 s to 0.01 s of an 84 s single-threaded
run. Below 64 sequences a position can take up to about 11 µs, at most 0.26 s on an extreme
alignment of 63 sequences with 30,000 positions. Only the last digits of Chao–Wang–Jost results
depend on this; every other estimator is unaffected.

## Similarity-weighted entropy

Similarity-weighted entropy weights each k-mer's entropy term by $1 - S_i$, where $S_i$ is the
frequency-weighted similarity of the k-mer to every other k-mer at the position. Summed over
pairs, that takes time proportional to the square of the number of distinct k-mers $D$: on the
HIV-1 pol DNA alignment (2,637 sequences, $k = 27$) it is 201,504,198 pairs and
$5.4 \times 10^9$ site comparisons. DiMA obtains the same sums from residue counts per site, in
time proportional to $D \cdot k$ ($4.3 \times 10^7$ table operations there). With $c_p(a)$ the
number of sequences whose k-mer has residue $a$ at offset $p$, the matches of k-mer $i$ with
every sequence at offset $p$ number $c_p(x_{ip})$: the column-profile form of sum-of-pairs
scores, with an identity score [@gotoh1994, eqs. (1) and (4a)]. Wu's sum runs over the other
k-mers ($j \ne i$), so the $k\,c_i$ matches with the k-mer's own $c_i$ copies are removed:

$$
\begin{equation}
a_i = \sum_p c_p(x_{ip}) - k\,c_i = k \sum_{j \ne i} c_j\, s_{ij} ,
\qquad 1 - S_i = \frac{kN - a_i}{kN} .
\end{equation}
$$

The numerators $kN - a_i$ are exact integers, summed per count class in 128-bit integers.
Besides the entropy terms $-\hat p \log_2 \hat p$, which are those of observed entropy, the only
rounding is one division and one multiplication per class and the compensated sum of the class
terms, taken in the same order as for observed entropy. So when every weight is 1 ($k = 1$, or
every pair of k-mers differs at every site) similarity-weighted entropy equals observed entropy
bit for bit, as long as $f_c \cdot k \cdot N \le 2^{53}$ for every class of $f_c$ k-mers seen $c$
times: integers up to $2^{53}$ convert exactly and their quotient is correctly rounded
[@goldberg1991, §2.1.4]. Weights of 1 need every pair to differ at the first site, so at most 26
distinct k-mers, and the bound holds whenever $kN \le 3.46 \times 10^{14}$.

Because it shares the entropy terms, similarity-weighted entropy also shares their rounding
error, the one that grows with $N$ at a conserved position (see
[Where the rounding error comes from](#where-the-rounding-error-comes-from)). The weighting
keeps that error but not the value: the dominant k-mer, whose term carries the error, keeps a
weight close to 1, while the accurate terms of the rare k-mers shrink to about $d/k$ of their
size. The absolute error is therefore the same as that of observed entropy, but the relative
error is larger, by up to the ratio $\hat H / H_\text{sim}$. Measured as in the table above,
with each rare k-mer differing from the dominant one at one site:

| Profile | $N$ | Observed entropy | Similarity-weighted, $k = 9$ | Similarity-weighted, $k = 27$ |
|---|---|---|---|---|
| one differing sequence | 2,208,211 | $2.2 \times 10^{-12}$ | $1.3 \times 10^{-11}$ | $2.2 \times 10^{-11}$ |
| near-conserved | 2,208,211 | $4.3 \times 10^{-13}$ | $2.5 \times 10^{-12}$ | $4.3 \times 10^{-12}$ |
| many doubletons | 2,208,211 | $1.6 \times 10^{-13}$ | $9.6 \times 10^{-13}$ | $1.6 \times 10^{-12}$ |
| one differing sequence | 17,500,000 | $3.0 \times 10^{-11}$ | $1.8 \times 10^{-10}$ | $3.3 \times 10^{-10}$ |
| near-conserved | 17,500,000 | $7.0 \times 10^{-12}$ | $4.3 \times 10^{-11}$ | $7.5 \times 10^{-11}$ |
| many doubletons | 17,500,000 | $2.4 \times 10^{-12}$ | $1.5 \times 10^{-11}$ | $2.5 \times 10^{-11}$ |

The absolute errors stay between $2 \times 10^{-17}$ and $7 \times 10^{-17}$ bits, as for observed
entropy, and the normalised values have the relative errors of the entropies they divide. The
standard deviation of similarity-weighted entropy from sampling, computed as for the entropies
above, is $9.8 \times 10^{-7}$ to $7.1 \times 10^{-6}$ bits on these profiles at 2,208,211
sequences and $1.3 \times 10^{-7}$ to $9.9 \times 10^{-7}$ bits at 17.5 million, so the rounding
error stays $3 \times 10^9$ to $2.2 \times 10^{11}$ times smaller. Longer k-mers shrink the rare
k-mers' terms further, so the relative error grows with $k$ towards about $1 + \ln N$ times that
of observed entropy: for one differing sequence at $k = 1{,}000$ it is $3.4 \times 10^{-11}$ at
2,208,211 sequences and $5.2 \times 10^{-10}$ at 17.5 million, still $1.9 \times 10^9$ times
below the sampling error, and no k-mer length goes much beyond. On the
four bundled datasets the largest relative error of similarity-weighted entropy is
$1.9 \times 10^{-14}$ (MERS-CoV spike gene, position 433, where 451 of 452 sequences carry one
27-mer). Mathematically both normalised values lie between 0 and 1 and similarity-weighted
entropy is at most the observed entropy; a normalised value that rounding puts above 1 by at most
$10^{-12}$ is written as 1, and a value outside these bounds by more than rounding stops the run
as an internal error (exit 1).

The computation counts the residues of $26 \times \min(k, 2048)$ table entries at a time, in
blocks of at most 2,048 sites, so its table stays at most 426 kB however long the k-mers are; it
also needs about 48 bytes per distinct k-mer, for each position being computed.

## What this means for testing

The differential test against the reference implementation
(`crates/dima-core/tests/reference.rs`) and the paper test (`crates/dima-core/tests/paper.rs`,
which compares the four bundled datasets with values the reference implementation computed)
share one rule: every number must agree within a relative $10^{-13}$, so a zero must be exactly
zero. Their alignments have at most about 11,000 sequences per position; the largest
difference measured is $2.6 \times 10^{-15}$ for the entropies and $1.9 \times 10^{-14}$ for
similarity-weighted entropy (see [Similarity-weighted entropy](#similarity-weighted-entropy)), a
last-bit difference in a platform's logarithm
moves the reference by at most $6 \times 10^{-16}$, and every legitimate code change measured
(summation order, a one-pass regression, the deferred fix below) moved the values by at most
$8 \times 10^{-15}$. The bound leaves a wide margin above all of these, yet catches a numerical
regression as small as $1.2 \times 10^{-13}$ (a binomial sum stopped too early). Floating-point issues make
small output differences unavoidable during development, and where results cannot be made
identical the accepted tolerance belongs in the documentation and the tests
[@taschuk2017, Rule 10].

At a million or more sequences per position the default estimator's own rounding reaches
$10^{-12}$ to $5 \times 10^{-11}$ relative, and similarity-weighted entropy's up to
$5.3 \times 10^{-10}$, so a test at that scale would need a looser rule or the deferred fix
below. No test runs at that scale.

## What the fix would be

The fix would make four changes; the first three were prototyped and measured against the
50-digit evaluations:

- the term $-p \log_2 p$ for $p \ge 1/2$ is computed as $-p\,\operatorname{log1p}((c - N)/N)/\ln 2$,
  where $c - N$ is exact, so nothing is lost to rounding near 1 [@goldberg1991, Theorem 4];
- the binomial odds come from the exact integers $c$ and $N - c$ instead of the rounded $p$;
- coverage uses a form without the cancellation when singletons dominate;
- $\alpha/2$ of the conserved-sequence Clopper–Pearson bound is written so that it is exactly
  $0.025$, which moves every bound by 1 or 2 units in the last place.

The reference implementation already uses the first two and the exact $\alpha/2$.

**Accuracy.** On 36 profiles of conserved positions with 200,000 to 17.5 million sequences,
every value came within $1.7 \times 10^{-15}$ of the exact values; today 11 to 15 of the values
per estimator are off by more than $10^{-13}$. Positions where no k-mer dominates keep about
$10^{-14}$ for `dima` at 10 million sequences, as today.

**Speed.** The default estimator ran as fast or slightly faster (0.90 to 1.02 times the time).

**Outputs.** The last one or two digits of many values change. Nothing printed to four decimals,
no conserved sequence and no summary line changes. Per bundled dataset, with the default
estimator:

| Dataset | Numbers changed | Largest relative change |
|---|---|---|
| MERS-CoV spike protein | 1,446 of 9,390 | $2.0 \times 10^{-15}$ |
| MERS-CoV spike gene | 5,582 of 47,086 | $3.4 \times 10^{-15}$ |
| HIV-1 *pol* protein | 456 of 41,156 | $1.1 \times 10^{-15}$ |
| HIV-1 *pol* gene | 1,048 of 800,270 | $6.1 \times 10^{-16}$ |

Five of the 13 golden output files of dima 1.0.0's command-line tests would change, and with
the exact $\alpha/2$ also the conserved-sequence tables.

## Why the fix is deferred

1. **No scientific result changes.** At every realistic $N$ the rounding error is at least
   $10^{10}$ times smaller than the sampling error ($1.9 \times 10^9$ times for similarity-weighted
   entropy). A fix would change digits that no analysis
   can rely on.
2. **The project's rule for engineering changes.** The paper is the specification, and a pure
   engineering change must leave results unchanged (see
   [Working agreements](/contributing/working-agreements/)). This fix changes stored digits, so
   it needs a reason that matters for results, and at realistic scale there is none.
3. **What the field asks for is analysis, not last-digit accuracy.** Darriba et al. ask
   developers to analyse round-off. They also show two runs that differed only in round-off
   giving different trees, which likelihood-based significance tests could not tell apart
   [@darriba2018]. HMMER justifies even reduced precision by a numerical stability analysis
   showing that the accumulated error is tolerable [@eddy2011]. This page is that analysis.
4. **Established practice.** Tools fix floating-point precision in ordinary releases, for example
   scikit-bio 0.2.3, bcftools 1.13 and SciPy 1.7.0 by their release notes. They promise
   identical numbers only for the same version: HMMER's user guide points to its archived
   versions "when exact reproducibility of an analysis is an issue". This is tool behaviour, with
   no peer-reviewed source. DiMA's own guarantee is the same: byte-identical results from the
   same version (see [Make results reproducible](/how-to/reproducibility/)). A later fix
   therefore breaks no guarantee; it needs a "Methodology" entry in the changelog.
5. **The one case with a practical cost is handled.** Chao–Wang–Jost's second term, which would
   cost time proportional to $N$ and lose up to 8 digits if summed as written, is evaluated
   exactly (see [Chao–Wang–Jost's second term](#chaowangjosts-second-term)).

The fix would be reconsidered when a test at a million or more sequences per position is added,
or when values are needed beyond ten significant digits.

## References

[^ref]
