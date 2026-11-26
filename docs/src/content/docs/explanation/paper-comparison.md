---
title: Comparison with the papers
description: DiMA's results next to the published values of its methods' source papers — the DiMA paper's HIV-1 pol and MERS-CoV spike datasets and Wu 2025's similarity-weighted entropy — with the reason for every difference.
---

The DiMA paper reports results for an HIV-1 *pol* gene alignment and for the MERS-CoV spike
protein of its Fig. 1C [@tharanga2025]. Both datasets are part of this repository (see
`tests/data/README.md` for their sources: the Los Alamos HIV Sequence Database [@kuiken2003] and
NCBI Virus [@hatcher2017]), so the comparison with the DiMA paper below is reproduced by every
test run and can be printed with

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
   the mean intercept of one run of the published procedure, before its negative-intercept rule, has
   a standard deviation of 0.0020 bits (exact)<!-- docs-numbers:hiv-run-mean:end -->. DiMA reports the exact expectation of the procedure's
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
[the methodology](/explanation/entropy/#similarity-weighted-entropy-opt-in)). Three of its examples
come with their data. Supplementary File 1 prints the four metrics of further simulated
populations (not those of Figs. 2 and 4). Supplementary File 2 lists the unique sequences, with
their read counts, of two samples of tomato brown rugose fruit virus (ToBRFV): one from tomato
plants with the susceptible tm-2 allele and one from plants with the resistant Tm-2² allele, both
of the 5′-120nts region, the first 120 nucleotides at the 5′ end of the genome
[@wu2025, Sections 2.2 and 3.3]; Supplementary File 3 gives the Hamming distance (the number of
sites at which two sequences differ) of every pair of them [@wu2025, Section 3.3]. The potato spindle tuber viroid (PSTVd) libraries of Fig. 5c
come from Wu's earlier study [@wu2025, Sections 2.3 and 3.2], whose S2 Table lists their unique sequences
with their read counts [@wu2024, Supporting information, S2 Table]. DiMA's values below are fixed
by the test suite (`crates/dima-core/tests/paper.rs`, which builds the simulated populations and
reads the ToBRFV and PSTVd data in `tests/data/`); the ToBRFV and PSTVd values and the Fig. 5c
pool also agree with the reference implementation, which computes $H_\text{sim}$ pair by pair as
Wu defines it. The tables, the Hamming distances, the changes between libraries and the
comparison with the S1 Table of Wu's earlier study are computed by
`scripts/reference/docs_numbers.py`, and the other readings of Fig. 5c below are arithmetic shown
in the text. $H$, $H_n$, $H_\text{sim}$ and $H_\text{nsim}$ are Wu's values, computed from the
observed frequencies: DiMA writes them as `observed_entropy`, `normalised_observed_entropy`,
`similarity_weighted_entropy` and `normalised_similarity_weighted_entropy`. Its `entropy` key, and
the first mean of the end-of-run line, are DiMA's corrected entropy, which Wu does not use, so
the runs below print an "entropy mean" a little above the $H$ of the tables.

**Supplementary File 1.** Its "Figure 2 Extended Data" sheet gives $H$, $H_n$, $H_\text{sim}$ and
$H_\text{nsim}$ to five decimals for five populations of 10 sequences, every pair of sequences at
one similarity, for 14 similarities from 0.999 to 0.001 (checked once against the spreadsheet,
when this page was written). DiMA's formulas reproduce 278 of its 280 values; the two others are
one cell, distribution 1 at similarity 0.3, which holds the values at 0.4. The tests check the
five populations at similarities 0.999, 0.9, 0.5 and 0.001 (built as ten 1,000-residue k-mers;
within $5 \times 10^{-6}$ of the printed values), and the reference implementation's self-test
checks distribution 1 at 0.9. These populations are not those of Fig. 2: the sheet's
distribution 1, for example, is 0.7, four times 0.05 and five times 0.02, Fig. 2's 0.9, 0.02 and
eight times 0.01. In the "Figure 3 Extended
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

**Fig. 5c: the PSTVd libraries.** Loop 27 of PSTVd, UUUUCA, was mutated to UNNNNA, each N any
of the four nucleotides [@wu2025, Fig. 5a]. The mutant pool was sequenced, and so were the
populations it gave in *Nicotiana benthamiana* plants: in the inoculated region (IR), at the
margin of the inoculated leaves (LM) and in systemic leaves (Sys), each in three plants, one
plant per replicate [@wu2024, Materials and methods]. Sheet "Loop27" of the S2 Table lists the
unique 6-nucleotide sequences of the ten libraries, in DNA letters (TTTTCA for UUUUCA), with
their read counts.
`tests/data/pstvd_loop27_<library>.fasta.bz2` holds each sequence once per read (library `pool`,
`ir1` to `ir3`, `lm1` to `lm3` and `sys1` to `sys3`), so a run such as

```sh
dima analyze -i tests/data/pstvd_loop27_pool.fasta.bz2 -a dna -k 6 --similarity-weighted-entropy -o pool.json
```

has one position whose k-mer counts are the read counts:

<!-- docs-numbers:pstvd:start -->

| Library | Reads | Unique sequences ($D$) | $H$ | $H_n$ | $H_\text{sim}$ | $H_\text{nsim}$ |
|---|---|---|---|---|---|---|
| Pool | 73,627 | 343 | 7.7128 | 0.9158 | 3.8754 | 0.4601 |
| IR-1 | 56,701 | 344 | 6.7745 | 0.8040 | 3.3020 | 0.3919 |
| IR-2 | 61,108 | 347 | 6.9968 | 0.8291 | 3.4247 | 0.4058 |
| IR-3 | 52,903 | 347 | 6.8783 | 0.8151 | 3.3456 | 0.3965 |
| IR, mean of the 3 plants | | | 6.8832 | 0.8161 | 3.3574 | 0.3980 |
| LM-1 | 30,013 | 245 | 1.8352 | 0.2312 | 0.8919 | 0.1124 |
| LM-2 | 33,075 | 270 | 1.1098 | 0.1374 | 0.5431 | 0.0672 |
| LM-3 | 38,708 | 225 | 0.9462 | 0.1211 | 0.4613 | 0.0590 |
| LM, mean of the 3 plants | | | 1.2971 | 0.1632 | 0.6321 | 0.0796 |
| Sys-1 | 62,182 | 216 | 0.9796 | 0.1263 | 0.4491 | 0.0579 |
| Sys-2 | 65,426 | 244 | 1.2365 | 0.1559 | 0.6131 | 0.0773 |
| Sys-3 | 50,835 | 248 | 0.7141 | 0.0898 | 0.2987 | 0.0376 |
| Sys, mean of the 3 plants | | | 0.9767 | 0.1240 | 0.4536 | 0.0576 |

<!-- docs-numbers:pstvd:end -->

Fig. 5c plots one point for the pool and one for each of IR, LM and Sys, without error bars; Wu
et al. give the IR, LM and Sys values of their own figures as means of the three replicates
[@wu2024, Fig. 2 legend]. Read from the figure's pixels, the practical pool's point agrees with the pool above, and the IR,
LM and Sys points with the means of their three plants, to within 0.06 bits for $H$ and
$H_\text{sim}$ and 0.005 for $H_n$ and $H_\text{nsim}$: about what the figure can be read to,
since its theoretical-pool points, whose values are exact, read up to 0.02 bits and 0.004 off.
No single LM plant would match: the LM point is at about 1.30 bits. The pool's values use every
listed sequence, <!-- docs-numbers:pstvd-pool:start -->
255 of them of the form UNNNNA and 88 others; the 255 alone would give $H_n = 0.9598$<!-- docs-numbers:pstvd-pool:end -->, against about 0.91 in the figure. As Section 3.2 reports
[@wu2025, Section 3.2], all four values fall from the pool through IR and LM to Sys. From the
pool to the mean of the Sys plants, <!-- docs-numbers:pstvd-drop:start -->
$H$ falls by 6.7361 bits (87.3%) and $H_\text{sim}$ by 3.4217 bits (88.3%), $H_n$ by 0.7918 (86.5%)
and $H_\text{nsim}$ by 0.4026 (87.5%)<!-- docs-numbers:pstvd-drop:end -->. So $H$ falls by more bits than $H_\text{sim}$, and $H_n$
by more than $H_\text{nsim}$, as the paper says; in proportion to the pool's values the four
fall by similar amounts.

The listed reads do not always add up to the library's total in the study's S1 Table, against
which the S2 Table's "reads per thousand" are computed: they are between 3 fewer and 2 more (the
pool lists 73,627 of 73,628 reads). Three libraries also list a different number of unique
sequences from S1 Table's: IR-1 one more, IR-2 three more and Sys-2 one fewer. The values here use
the listed sequences and reads; Wu 2025 does not say which it used, and the figure cannot tell
such small differences apart. S1 Table also prints each library's normalised Shannon entropy.
Wu et al. write it as the entropy divided by $\log_2$ of the number of reads
[@wu2024, Results and Materials and methods], but the printed values are the entropy divided by
$\log_2$ of the number of unique sequences, $H_n$ (divided by $\log_2$ of the reads, the pool's
would be 0.48): <!-- docs-numbers:pstvd-s1:start -->
IR-1's 0.803969923 is $H_n$ above to its last decimal, and the others differ from $H_n$ by at most
0.0008 (IR-2)<!-- docs-numbers:pstvd-s1:end -->. IR-1 is the one library whose listed reads add up to
its S1 total.

**ToBRFV.** Supplementary File 2 lists 48 (tm-2 sample) and 47 (Tm-2² sample) unique
120-nucleotide sequences of the 5′-120nts region with their read counts. Wu reports excluding
sequences below 0.1% of the reads, but the tm-2 list holds eight with 10 of 10,833 reads (0.092%);
all of them are used here. `tests/data/tobrfv_tm2.fasta.bz2` and `tobrfv_tm22.fasta.bz2` hold each
sequence once per read, so a run such as

```sh
dima analyze -i tests/data/tobrfv_tm2.fasta.bz2 -a dna -k 120 --similarity-weighted-entropy -o tm2.json
```

has one position whose k-mer counts are the read counts:

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

The tm-2 values match Fig. 6d, reading its $H_\text{nsim}$ axis as linear (its tick labelled
0.015 stands where 0.010 would). For Tm-2², Fig. 6d shows about 0.71, 0.127, 0.116 and 0.021: with
the published data $H$ and $H_n$ are higher in Tm-2² than in tm-2, the opposite of Section 3.3,
while $H_\text{sim}$ and $H_\text{nsim}$ are higher in Tm-2², as the paper reports. The mean and
largest Hamming distances match Section 3.3, and the committed sequences reproduce every distance
of Supplementary File 3 (2,304 pairs for tm-2 and 2,209 for Tm-2², the sequences taken by their
`SeqN` IDs; checked once against the spreadsheet, when this page was written). As of 25 November 2026 Crossref lists no correction
to the paper.

## References

[^ref]
