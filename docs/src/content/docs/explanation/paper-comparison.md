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
   slightly different mean; three runs of the published procedure gave 5.350 to 5.353. DiMA
   reports the exact expectation of the procedure's intercept, 5.3504 (see
   [The exact expectation](/explanation/entropy/#the-exact-expectation)).
2. **Range and highest position.** The paper's extremes come from one random run. The expected
   corrected entropies at positions 2176 and 2177 are 9.4755 and 9.4601 bits, a difference well
   within the spread of the random procedure at those positions (a standard deviation of about
   0.07 bits), so either position can come out highest in a single run.
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
5. **Fig. 1C.** The figure shows one draw of the random procedure. Its standard deviation at this
   position is 0.22 bits, and 1.38 lies at about the 39th percentile of its distribution; the
   expectation is 1.4451 (1.4617 with the default $T = 100$). See
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
near-conserved positions; averaged over many of its runs, the MERS-CoV spike protein means are
about 0.142 bits ($T = 100$) and 0.132 bits ($T = 30$), the gene's about 0.384 bits, and the
HIV-1 *pol* means are unchanged (see
[the negative-intercept rule](/explanation/entropy/#the-negative-intercept-rule)).

## Similarity-weighted entropy (Wu 2025)

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

## References

[^ref]
