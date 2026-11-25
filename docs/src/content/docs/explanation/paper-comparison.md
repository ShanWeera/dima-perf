---
title: Comparison with the papers
description: DiMA's results next to the published values of its methods' source papers — the DiMA paper's HIV-1 pol and MERS-CoV spike datasets and Wu 2025's similarity-weighted entropy — with the reason for every difference.
---

The DiMA paper reports results for an HIV-1 *pol* gene alignment and for the MERS-CoV spike
protein of its Fig. 1C [@tharanga2025]. Both datasets are part of this repository (see
`tests/data/README.md` for their sources: the Los Alamos HIV Sequence Database [@kuiken2003] and
NCBI Virus [@hatcher2017]), so the comparison below is reproduced by every test run and can be
printed with

```sh
scripts/validate_paper.sh            # builds target/release/dima if needed
```

## Results

All values with the defaults: $T = 100$, $k = 27$ (DNA) or 9 (protein), the `dima` estimator.

| Quantity | Paper | DiMA | Note |
|---|---|---|---|
| HIV-1 *pol* DNA: positions | 3,364 | 3,364 | |
| HIV-1 *pol* DNA: LS positions | 564 | 564 | |
| HIV-1 *pol* DNA: NS positions | 680 | 680 | |
| HIV-1 *pol* DNA: mean entropy (support > $T$) | ~5.35 | 5.3504 | (1) |
| HIV-1 *pol* DNA: entropy range | ~0.86 to ~9.57 | 0.8976 to 9.4755 | (2) |
| HIV-1 *pol* DNA: highest-entropy position | 2177 | 2176 | (2) |
| HIV-1 *pol* DNA: lowest total variants incidence | position 3046 | position 3046 (12.842%, tied with 3047) | |
| HIV-1 *pol* DNA: conserved sequences at 70% | two, of 36 and 34 nt | two: columns 3046–3077 (32 nt, incidence 80.1–87.2%) and 3156–3185 (30 nt, 71.5–74.3%) | (3) |
| HIV-1 *pol* protein: mean entropy (support > $T$) | ~1.91 | 1.9072 | (4) |
| MERS-CoV spike position 25: support | 170 | 170 | |
| MERS-CoV spike position 25: distinct 9-mers | 10 | 10 | |
| MERS-CoV spike position 25: Index | `SVKSACIEV` 129 (75.88%) | `SVKSACIEV` 129 (75.88%) | |
| MERS-CoV spike position 25: distinct variants incidence | 22.0% | 21.95% | |
| MERS-CoV spike position 25: entropy ($T = 30$) | 1.38 | 1.4451 | (5) |

## Notes

1. **Mean entropy.** The paper's correction draws random subsamples, so each run gives a
   slightly different mean; <!-- docs-numbers:hiv-run-mean:start -->
   the mean of one run of the published procedure has a standard deviation of 0.0020 bits (exact)<!-- docs-numbers:hiv-run-mean:end -->. DiMA reports the exact expectation of the procedure's
   intercept, 5.3504 (see [The exact expectation](/explanation/entropy/#the-exact-expectation)).
2. **Range and highest position.** The paper's extremes come from one random run. <!-- docs-numbers:hiv-2176-2177:start -->
   The expected corrected entropies at positions 2176 and 2177 are 9.4755 and 9.4601 bits, a
   difference well within the spread of the random procedure at those positions (exact standard
   deviation 0.0661 bits at both)<!-- docs-numbers:hiv-2176-2177:end -->, so either position can come out highest in a single
   run.
3. **Conserved-sequence lengths.** DiMA finds two regions, as the paper does, each four
   nucleotides shorter than the published lengths. DiMA v5.0.9's own conserved-sequence code, applied to the same
   alignment, also gives 32 and 30 nt; the extra four nucleotides cannot be reproduced from the
   published code, and the web server the paper used is no longer online. Both regions are
   unflagged (every member has support above $T$) and are the same with `--hcs-eligible-only`;
   their 95% Clopper–Pearson lower bounds are 78.5% and 69.8%. The paper also notes that "one
   HCS was observed at ∼88%" without saying which statistic it reports; DiMA's first region has
   Index incidences from 80.1% to 87.2%.
4. **Stop and frameshift symbols.** The HIV-1 *pol* protein alignment from the Los Alamos
   database contains `*` (stop) and `#` (frameshift). DiMA excludes them like gaps; counted
   as residues, as DiMA v5.0.9 did, they give 1.9125. Both round to the paper's 1.91.
5. **Fig. 1C.** The figure shows one draw of the random procedure. <!-- docs-numbers:fig1c:start -->
   Its exact standard deviation at this position is 0.2241 bits, and 1.38 lies 0.29 standard
   deviations below the expectation, 1.4451 (1.4617 with the default $T = 100$, standard deviation
   0.3047)<!-- docs-numbers:fig1c:end -->. See
   [Entropy and its correction](/explanation/entropy/#the-exact-expectation).

## Other datasets

The repository also contains a MERS-CoV spike *gene* alignment (499 sequences, 4,065 columns,
lower case) for which the paper reports no numbers. DiMA's values on all four datasets are fixed
by the test suite (`crates/dima-core/tests/paper.rs`) and agree with the independent reference
implementation (`scripts/reference/`) within a relative $10^{-13}$ (see
[Numerical accuracy](/explanation/numerical-accuracy/#what-this-means-for-testing)):

| Dataset ($k$) | Positions | NS / LS / ELS / eligible | Mean (eligible) | Lowest | Highest (position) |
|---|---|---|---|---|---|
| MERS-CoV spike protein (9), $T = 30$ | 1,345 | 0 / 0 / 0 / 1,345 | 0.1235 | 0 | 1.5694 (26) |
| MERS-CoV spike protein (9) | 1,345 | 0 / 0 / 0 / 1,345 | 0.1261 | 0 | 1.5878 (26) |
| MERS-CoV spike gene (27) | 4,039 | 0 / 29 / 0 / 4,010 | 0.3824 | 0 | 1.5705 (1415, 2 tied) |
| HIV-1 *pol* protein (9) | 1,189 | 551 / 162 / 0 / 476 | 1.9072 | 0.0768 | 5.9658 (772) |
| HIV-1 *pol* DNA (27) | 3,364 | 680 / 564 / 0 / 2,120 | 5.3504 | 0.8976 | 9.4755 (2176) |

DiMA v5.0.9 applies the negative-intercept rule to each random run, which raises its average at
near-conserved positions; averaged over many of its runs, <!-- docs-numbers:v509-means:start -->
the MERS-CoV spike protein means are 0.1417 bits ($T = 100$) and 0.1317 bits ($T = 30$) and the
gene's 0.3842 bits (seeded Monte Carlo of 2,000, 2,000 and 200 runs, 95% intervals ±0.0001), and the
HIV-1 *pol* means differ from DiMA's by less than 0.00026 bits (DNA) and 0.0011 bits (protein)<!-- docs-numbers:v509-means:end --> (see
[the negative-intercept rule](/explanation/entropy/#the-negative-intercept-rule)).

## Similarity-weighted entropy (Wu 2025)

Wu's paper introduces similarity-weighted entropy [@wu2025] (see
[the methodology](/explanation/entropy/#similarity-weighted-entropy-opt-in)). Two of its examples
come with their data: Supplementary File 1 prints the four metrics of the simulated populations,
and Supplementary File 2 lists the unique ToBRFV sequences of the two plants with their read
counts. Wu's potato spindle tuber viroid (PSTVd) pools (practical, IR, LM and Sys) are not
published as sequences. Every value below is fixed by the test suite
(`crates/dima-core/tests/paper.rs`, which builds the simulated populations and reads the ToBRFV
data in `tests/data/`) and agrees with the reference implementation, which computes
$H_\text{sim}$ pair by pair as Wu defines it.

**Supplementary File 1.** Its "Figure 2 Extended Data" sheet gives $H$, $H_n$, $H_\text{sim}$ and
$H_\text{nsim}$ to five decimals for five populations of 10 sequences, every pair of sequences at
one similarity, for 14 similarities from 0.999 to 0.001. DiMA's formulas reproduce 278 of its 280
values; the two others are one cell, distribution 1 at similarity 0.3, which holds the values at
0.4. The tests check the five populations at similarities 0.999, 0.9, 0.5 and 0.001 (built as ten
1,000-residue k-mers; within $5 \times 10^{-6}$ of the printed values). In the "Figure 3 Extended
Data" sheet the cell of D-1 at similarity 0.7 holds the values at 0.95 ($H_\text{sim}$ 0.01921 and
$H_\text{nsim}$ 0.00578; at 0.7 they are 0.04363 and 0.01313 for the frequencies of Fig. 3a).

**Fig. 4: saturated pools.** Every sequence of length $k$ (1 to 7), each once, gives
$H = 2k$ bits, $H_n = 1$, $H_\text{nsim} = 3/4 + 4^{-k}$ and $H_\text{sim} = 2k(3/4 + 4^{-k})$:
$H_\text{nsim}$ falls from 1 at $k = 1$ to 0.8125, 0.7656, … and $H_\text{sim}$ reaches 10.5009
bits at $k = 7$, as the figure shows. (The caption swaps the panels; see
[Where the paper and DiMA differ](/explanation/entropy/#where-the-paper-and-dima-differ).)

**Fig. 5c: the theoretical UNNNNA pool.** The 256 sequences UNNNNA at equal frequencies (four of
six sites free) give $H = 8$, $H_n = 1$, $H_\text{sim} = 129/32 = 4.03125$ and
$H_\text{nsim} = 129/256 = 0.50390625$, where the paper prints 8, 1, 4.04 and 0.50. The printed
4.04 is reproduced if the similarities are rounded to two decimals first (4.0353); the paper does
not say so. Three decimals give 4.0317; counting each sequence's similarity to itself gives 4.00,
and the similarities of Fig. 5b, which are those of the four free sites only, give 6.03.

**ToBRFV.** Supplementary File 2 lists 48 (tm-2 plant) and 47 (Tm-2² plant) unique 120-nucleotide
sequences of the 5′-120nts region with their read counts (Wu excluded sequences below 0.1% of the
reads). `tests/data/tobrfv_tm2.fasta.bz2` and `tobrfv_tm22.fasta.bz2` hold each sequence once per
read, so a run with `-a dna -k 120 --similarity-weighted-entropy` has one position whose k-mer
counts are the read counts:

<!-- docs-numbers:tobrfv:start -->

| Quantity | tm-2 | Tm-2² |
|---|---|---|
| Reads | 10,833 | 6,969 |
| Unique sequences ($D$) | 48 | 47 |
| $H$ | 0.7320 | 0.7785 |
| $H_n$ | 0.1311 | 0.1402 |
| $H_\text{sim}$ | 0.0961 | 0.1056 |
| $H_\text{nsim}$ | 0.0172 | 0.0190 |
| Mean Hamming distance between unique sequences | 2.12 | 2.80 |
| Largest Hamming distance between unique sequences | 5 | 10 |

<!-- docs-numbers:tobrfv:end -->

The tm-2 values match Fig. 6d. For Tm-2², Fig. 6d shows about 0.71, 0.127, 0.116 and 0.021: with
the published data $H$ and $H_n$ are higher in Tm-2² than in tm-2, the opposite of Section 3.3,
while $H_\text{sim}$ and $H_\text{nsim}$ are higher in Tm-2², as the paper reports. The mean and
largest Hamming distances match Section 3.3. As of 25 November 2026 Crossref lists no correction
to the paper.

## References

[^ref]
