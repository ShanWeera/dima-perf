---
title: Diversity motifs and summaries
description: How DiMA classifies the variants of a position as Index, Major, Minor and Unique motifs, the incidences it reports, when a ratio is undefined, and how the run summaries are formed.
---

Entropy condenses a position into one number. **Diversity motifs** keep the structure: which
variant dominates, which variants follow, and how much of the population carries something other
than the dominant variant. The motif classes were introduced by Hu et al. for HIV-1 [@hu2013] and
applied by Abd Raman et al. to influenza A (H5N1) [@raman2020], as the paper recounts; DiMA
reports them as the paper defines them [@tharanga2025].

## Motif classes

At a position with variant counts $c_1 \ge c_2 \ge \dots \ge c_D$:

| Motif | Short | Rule |
|---|---|---|
| Index | `I` | the highest count, if it is above 1 |
| Unique | `U` | count 1 |
| Major | `Ma` | the most common of the other variants seen more than once |
| Minor | `Mi` | every other variant seen more than once |

The short codes are the values of `motif_short` (and the desktop app's variant search accepts
them); `motif_long` spells the motif out.

Ties are kept, as the paper states that "a position may exhibit more than one index or major
variant": every variant tied at the highest count is Index, and every variant tied at the
next-highest count is Major. A position where every variant occurs once has no Index (all are
Unique). Examples, by count:

| Counts | Motifs |
|---|---|
| 129, 14, 9, 6, 4, 4, 1, 1, 1, 1 | I, Ma, Mi, Mi, Mi, Mi, U, U, U, U |
| 3, 3, 2, 2, 1 | I, I, Ma, Ma, U |
| 7, 4, 3, 3 | I, Ma, Mi, Mi |
| 5 | I |
| 1, 1, 1 | U, U, U |

Variants are listed by count, highest first; variants with equal counts are ordered by their
sequence (byte order), so the order is the same in every run.

## Incidences

All incidences are percentages of the position's support $N$ or of a part of it:

$$
\begin{equation}
\text{incidence}_i = 100 \, \frac{c_i}{N} ,
\label{eq:incidence}
\end{equation}
$$

$$
\begin{equation}
\text{total variants incidence} = 100 \, \frac{N - \sum_{\text{Index}} c_i}{N} ,
\label{eq:total}
\end{equation}
$$

$$
\begin{equation}
\text{distinct variants incidence} = 100 \,
  \frac{\text{number of non-Index variants}}{N - \sum_{\text{Index}} c_i} .
\label{eq:distinct}
\end{equation}
$$

The **total variants incidence** $\eqref{eq:total}$ is the share of sequences that carry a
variant other than the Index; the **distinct variants incidence** $\eqref{eq:distinct}$ measures
how many different variants those sequences carry, relative to how many sequences there are. The
number of non-Index variants is reported as `distinct_variants_count`.

At MERS-CoV spike position 25 (support 170; the Index `SVKSACIEV` occurs 129 times):
41 sequences carry a non-Index variant, so the total variants incidence is
$100 \cdot 41/170 = 24.12\%$; those 41 sequences carry 9 distinct variants, so the distinct
variants incidence is $100 \cdot 9/41 = 21.95\%$ (the paper's 22.0%).

The paper's text gives this denominator — the total variants count — and so does its Fig. 1C.
Its schematic Fig. 1A divides by the support instead ("Distinct variants (6/20)"); DiMA follows
the text and Fig. 1C [@tharanga2025].

### Undefined ratios

A ratio whose denominator is 0 is undefined, and DiMA reports it as `null` (JSON) or `NA`
(tables) instead of inventing a value:

- a position **without support** has no variants (`diversity_motifs` is empty) and both
  incidences are `null`;
- a **fully conserved** position (every sequence carries the Index) has a total variants
  incidence of 0 and an undefined distinct variants incidence ($0/0$).

The second case does not mean that the position is conserved: a position whose variants are all
tied Index variants gives the same two values. Position 6 of the repository's tiny test alignment
(`tests/golden/tiny/input.fasta`) has two 3-mers, `IAK` and `LAK`, carried by 4 sequences each:
both are Index, so no sequence carries a non-Index variant, the total variants incidence is 0 and
the distinct variants incidence is `null` — while its entropy is 1.0742 bits (1 bit observed).
Read the incidences together with the entropy or `diversity_motifs`.

DiMA v5.0.9 reported both cases as 0, which made a conserved position indistinguishable from a
position with no data.

## Metadata per variant

With `--header-format`, each variant also carries the number of its sequences per value of every
stratified header field — for example how many of the 14 `SFKSACIEV` sequences at position 25
came from each host. Fields appear in header-format order and values in byte order. See
[Stratify by header metadata](/how-to/metadata/).

## Run summaries

Every result file starts with run-level summaries.

**Eligible summary** (top-level keys). The mean, lowest and highest entropy over the eligible
positions (support above $T$) — exactly the quantities the paper reports for its datasets: the
mean entropy, the range and the position of the highest entropy [@tharanga2025]. The extrema are
reported with the **first** position that has the value and the number of tied positions
(`tied_position_count`), so ties are visible instead of being resolved silently. All three are
`null` when no position is eligible. The mean is the
[eligible mean](#the-eligible-mean), explained below.

**All-positions summary** (`all_positions_summary`). The same statistics with DiMA v5.0.9's
averaging rule — every position, NS positions counted as 0 bits and LS/ELS positions with their
reported entropy (the observed entropy with the `dima` and `raw` estimators) — so that results can
be compared with earlier analyses. The `rule` key records
the rule. It is not the paper's summary; see
[why NS positions have no entropy](/explanation/entropy/#positions-without-support). Because NS
positions count as 0 bits, its lowest entropy can be a position without any data: on the HIV-1
*pol* protein alignment it is position 30, at 0 bits with 671 other positions, 551 of the 672
being NS.

**Support classes.** `position_count`, `eligible_position_count`, the counts per label
(`support_label_counts`: NS, LS, ELS, eligible) and `low_support_count` (NS + LS + ELS).

**Coverage summary** (`coverage_summary`). How many positions have a sample coverage (support of
at least 2), its minimum and mean, and with `--min-coverage` the threshold and the number of
positions below it. Unlike the eligible summary, it covers LS and ELS positions too, so its
minimum can come from a position with very few sequences: on the HIV-1 *pol* protein alignment
it is 0.1818, at LS position 29, whose 4 sequences carry 4 different 9-mers; over the 476
eligible positions the lowest coverage is 0.7338.

**Similarity-weighted entropy summary** (`similarity_weighted_entropy_summary`). Only with
`--similarity-weighted-entropy`, and `null` otherwise: the mean, lowest and highest
[similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in)
[@wu2025] over the eligible positions, and the mean *observed* entropy of the same positions, so
that the two can be compared like for like. The normalised values are not summarised.

For the MERS-CoV spike protein with the defaults: 1,345 positions, all eligible; mean entropy
0.1261 bits; lowest 0 bits (first at position 7, 455 positions tied); highest 1.5878 bits at
position 26. With `--similarity-weighted-entropy`, the mean similarity-weighted entropy is
0.0363 bits, against a mean observed entropy of 0.1207 bits over the same positions; the lowest
is 0 bits (first at position 7, the same 455 positions tied, each with a single variant) and the
highest 0.4548 bits, again at position 26.

### The eligible mean

The mean of the eligible summary, `average_entropy`, is called the **eligible mean** in the
desktop app, after the positions it covers. With $E$ the set of eligible positions and $H_p$ the
reported entropy of position $p$ (`entropy`: the paper's corrected entropy by default, or the
value of the estimator chosen with `--estimator`),

$$
\begin{equation}
\bar H_E = \frac{1}{|E|} \sum_{p \in E} H_p ,
\label{eq:eligible-mean}
\end{equation}
$$

where $|E|$ is `eligible_position_count`. Each position counts once, whatever its support: with
$T = 100$, a position supported by 3,000 sequences weighs as much as one supported by 101. The
same number is `entropy mean` in the line `dima analyze` prints at the end of a run, the *mean*
chip of the desktop app, and the dashed line labelled *eligible mean* on the app's entropy chart.
When that chart shows similarity-weighted entropy, the same line is the eligible mean of
similarity-weighted entropy instead: $\eqref{eq:eligible-mean}$ with the similarity-weighted
entropy in place of $H_p$, which is `similarity_weighted_entropy_summary.average_entropy` and the
first number of the app's similarity-weighted chip (0.0363 bits on the MERS-CoV spike sample).

**Worked example.** The HIV-1 *pol* protein data set
(`tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2`, the default options) has 1,189 positions: 476
eligible, 162 LS and 551 NS. Its eligible mean is the average of the 476 eligible entropies,
1.9072 bits; the paper reports about 1.91 bits for HIV-1 *pol* protein (see
[Comparison with the papers](/explanation/paper-comparison/)). The table compares it with three
other averages of the same run:

| Average over | Positions | Mean entropy (bits) |
|---|---|---|
| eligible positions (the eligible mean, `average_entropy`) | 476 | 1.9072 |
| every position with an entropy, eligible and LS (what the app's chart draws) | 638 | 1.5708 |
| LS positions alone | 162 | 0.5823 |
| every position, NS as 0 (`all_positions_summary`, above) | 1,189 | 0.8429 |

The eligible mean leaves out the 162 LS positions, which rest on fewer than $T = 100$
sequences and keep their uncorrected observed entropy, and the 551 NS positions, which have no
entropy at all: it covers the positions of the paper's summaries, those with support above $T$
[@tharanga2025, Results and Fig. 2].

It includes every eligible position with the entropy reported for it, corrected or not. Two of
these 476 positions, 42 and 43, are supported by 105 sequences, fewer than the 125 the paper's
correction needs at $T = 100$, so they enter the eligible mean with their observed entropies,
4.0041 and 3.5039 bits (`entropy_fallback` `too_few_points`; see
[the paper's correction](/explanation/entropy/#the-papers-correction-fig-2)). The warning
[`uncorrected_eligible_positions`](/reference/diagnostics/#uncorrected_eligible_positions) says how
many such positions a run has.

A mean does not say where the diversity lies. On the MERS-CoV spike sample every position is
eligible and the eligible mean is 0.1261 bits, while 455 of its 1,345 positions have entropy 0
and the highest has 1.5878 bits; the lowest and highest entropy of the summary, and the values
of the positions, show that.

The eligible mean also depends on the run's settings. On the same sample it is 0.1207 bits with
`--estimator raw` (the observed entropy), and 0.1235 bits with `-s 30`, although every position
is eligible at both thresholds, because the subsample sizes of the paper's correction depend on
$T$ (see
[the paper's correction](/explanation/entropy/#the-papers-correction-fig-2)).
Before you compare the eligible means of two alignments, see
[Can I compare diversity between proteins or datasets?](/getting-started/faq/#can-i-compare-diversity-between-proteins-or-datasets).

## References

[^ref]
