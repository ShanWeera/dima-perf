---
title: Historically conserved sequences
description: How DiMA finds historically conserved sequences (HCS), joins positions into regions, flags regions resting on few sequences and bounds their conservation with an exact Clopper–Pearson interval.
---

Stretches of a viral proteome that stay invariant across years of evolution are candidate
targets for vaccines and diagnostics [@khan2008; @rolland2007]. DiMA reports them as
**historically conserved sequences** (HCS): runs of positions whose most common k-mer is carried
by (almost) every sequence [@tharanga2025; @hu2013; @raman2020].

## Which positions qualify

A position qualifies when its **Index** variant (see [motifs](/explanation/motifs/)) reaches the
threshold incidence $\theta$ (`--hcs-min-incidence`, default 100%, the paper's "completely
conserved"):

$$
\begin{equation}
100 \cdot c_\text{Index} \ \ge\ \theta \cdot N .
\label{eq:qualify}
\end{equation}
$$

The comparison is exact: $\theta$ is read as a decimal (for example 95.5) and compared in integer
arithmetic, so a position never misses the threshold through floating-point rounding.

$\theta$ must lie above 50% and at most 100%. Above 50% at most one variant can reach it, so
every qualifying position has exactly one conserved k-mer and a region's sequence is well defined.
(DiMA v5.0.9 accepted any threshold and let tied Index variants of the same position compete.)
Because an Index needs a count above 1, a qualifying position has a support of at least 2.

## Joining positions into regions

Qualifying positions are taken in order, and each one joins the current region when

1. its k-mer **overlaps or abuts** the region: it starts no later than one column after the
   region's last column, as in the conserved-sequence tables of Abd Raman et al. [@raman2020]; and
2. it **agrees** with the region on every column they share.

Otherwise a new region starts; when the new k-mer overlapped the previous region but disagreed
with it, the new region is marked `split_from_previous`. Consecutive overlapping 9-mers therefore
build one region whose sequence is the union of their columns: positions 7–14 of the MERS-CoV
spike protein form the 16-residue region `LLMFLLTPTESYVDVG`, columns 7–22.

Each region reports its columns (`start_column`, `end_column`, `length`), its sequence, its
member positions with their k-mer, count, support and incidence, the lowest member incidence
(`min_incidence`) and the lowest member support (`min_support`); what these numbers say about the
region as a whole is explained [below](#what-a-regions-numbers-mean).

## Flagging regions that rest on few sequences

The paper, like DiMA v5.0.9, lets any position qualify, whatever its support. A k-mer carried by
2 of 2 sequences is "100% conserved", but that says little. DiMA keeps every qualifying position
and **flags** the regions that include positions with support at or below $T$, as the DiMA
lineage did: Abd Raman et al. list such positions in their conserved-sequence tables, marked with
an asterisk ("Positions with total number of sequences less than 100") [@raman2020], and Hu et
al. show them "to maintain the protein sequence structure" [@hu2013]. Each region lists
`low_support_positions` — every position from its first member to its last with support
$\le T$, member or not — and is `flagged` when the list is not empty. Flagged regions raise the
`hcs_low_support_regions` warning.

### A lower bound on conservation

To make "how conserved, on how much evidence" explicit, every member and every region carries the
exact two-sided 95% **Clopper–Pearson** lower confidence bound of the Index proportion
[@clopper1934]: for $x$ Index sequences out of $N$, the proportion $p$ at which observing $x$ or
more has probability 2.5%,

$$
\begin{equation}
\Pr\left[X \ge x \mid X \sim \operatorname{Binomial}(N, p)\right] = 0.025 ,
\qquad\text{and}\qquad p = 0.025^{1/N} \ \text{ when } x = N .
\label{eq:cp}
\end{equation}
$$

A region's `conservation_lower_bound_95` is the lowest per-position bound among its members, not a
bound on the share of sequences that carry the whole region (see
[what a region's numbers mean](#what-a-regions-numbers-mean)). The interval is conservative — its
coverage probability, the chance that it contains the true proportion, is at least the nominal 95%
[@brown2001] — so a high bound is strong evidence. (This coverage of an interval has nothing to do
with the [sample coverage](/reference/glossary/#sample-coverage) of a position.) For a k-mer
carried by every sequence:

| Support $N$ | 2 | 3 | 10 | 17 | 100 |
|---|---|---|---|---|---|
| Lower bound | 15.8% | 29.2% | 69.2% | 80.5% | 96.4% |

A bound of at least 80% needs at least 17 sequences.

### Eligible positions only

`--hcs-eligible-only` admits only positions with support above $T$, and lets a position with
support $\le T$ between two members end the region instead of being flagged. This is the
analysis set of Hu et al. [@hu2013], and the behaviour of alignment-trimming tools that let
gappy columns break conserved blocks [@castresana2000]. Use it when a conserved region must rest
on well-supported positions only.

## What a region's numbers mean

A region's numbers describe its **member positions**, one at a time: `min_incidence`,
`min_support` and `conservation_lower_bound_95` are the lowest values among the members, and every
member's Index reaches the threshold $\theta$. They say less about the region as a whole than they
seem to, in three ways.

**Positions between members need not qualify.** A k-mer that only overlaps or abuts a region
joins it, so the region's columns also hold the windows of positions that did not reach
$\theta$. On the MERS-CoV spike protein at 95%, region 3 spans columns 195–458, with 242 members
from position 195 to 450, a `min_incidence` of 95.00% and a lower bound of 0.9072. The 14 positions
220–224 (92.5–94.8%) and 403–406, 408–411 and 416 (91.1–94.5%) lie inside it without being
members. At position 220 two sets of rare variants meet: those of member 219, at columns 221, 222
and 224 (5 sequences), and those of member 225, at column 228 (8 sequences). Its window, columns
220–228, holds both, so 13 of its 174 sequences carry a non-Index 9-mer: 92.5%, while each of the
two members stays above 95%.

**Few sequences may carry the whole region.** Different sequences can differ from the Index at
different members, so the share of sequences that carry a region's entire sequence can be far
below `min_incidence`. Of the 166 MERS-CoV sequences with a residue at every column of region 3,
only 107 (64.5%) carry its whole 264-residue sequence. At 95% the 11 regions are carried by
64.5–98.2% of the sequences complete over them; at 90%, region 2 spans columns 29–1192 and is
carried by 12 of 149 (8.1%). On the HIV-1 *pol* DNA alignment at 70%, region 1 (columns
3046–3077, `min_incidence` 80.1%) is carried by 2,093 of the 2,632 complete sequences (79.5%).
At 100% the two agree, by arithmetic: every sequence with a valid k-mer at a member carries its
Index, so every sequence complete over a region carries all of it (all 44 regions of the MERS-CoV
spike protein). DiMA does not report this share: regions are computed from the results, which
hold counts per position, not the sequences. [Find conserved sequences](/how-to/conserved-sequences/)
gives a recipe that counts the carriers in the alignment.

**Regions can share columns.** A region that is `split_from_previous` starts inside the previous
one: on the MERS-CoV spike gene at 95%, regions 34 (columns 3473–3543), 35 (3518–3572) and 36
(3547–3579) overlap. Adding up `length` then counts the shared columns twice: the 39 regions'
lengths sum to 3,861, but they cover 3,809 distinct columns.

The DiMA lineage reads its conserved sequences the same way. Abd Raman et al. note that "not all
amino acids of the concatenated sequence may meet the incidence criteria set for the index
nonamer sequence", and their table marks such positions: "Positions that do not satisfy the
concatenating criteria are indicated as small letter case" [@raman2020, Methods and Table 3]; Hu
et al. give their statistics per nonamer position [@hu2013, Results].

Per-position agreement overstates agreement over a longer stretch, as the lineage measured
directly: individual HIV-1 sequences matched the historical Index at more than 82% of single
amino acids, but only 61% (HXB2) and 49% (C1P) of their 9-mers matched the Index 9-mers, so
"approximately half of the overlapping nonamer sequences of a given HIV-1 strain can be expected
to differ from the historical index sequences" [@hu2013, Results]. Hu et al. also describe
**index switching**: positions where the most common k-mer is not the continuation of the
preceding position's, revealed by "amino acid mutations that did not follow the consecutive
concatenation of the index sequences" [@hu2013, Results]. Where an overlapping k-mer disagrees
with a region, DiMA starts a new one and marks it `split_from_previous`.

When the share of sequences carrying a whole stretch is the question, it is a measure of its own:
in epitope analysis, conservancy is "the fraction of protein sequences that contain the epitope
considered at or above a specified level of identity" [@bui2007, Background], and the DiMA
lineage selected its pan-dengue sequences that way, keeping only "sequence fragments that were
identical in at least 80% of the sequences of each of the 4 types" [@khan2008, Methods]. So
below 100%, read `min_incidence` as an upper bound on the share of the alignment's sequences that
carry the whole region (no more sequences can carry it than carry the Index at any one member, and
each member's support is at most the number of sequences), say so when you report it, and count
the carriers in the alignment when that share matters (the recipe in [Find conserved sequences](/how-to/conserved-sequences/)). No published
rule converts per-position incidences into a whole-region share: the positions are not
independent, so multiplying their incidences would be wrong.

The member positions, their k-mers and bounds are in the conserved-sequences JSON (`members`);
the TSV table gives the region-level values only (see [TSV tables](/reference/tsv/#hcs)).

## Examples

With the defaults ($T = 100$; HIV-1 *pol* DNA at 70%):

| Dataset | Threshold | Regions | Flagged | Notes | With `--hcs-eligible-only` |
|---|---|---|---|---|---|
| MERS-CoV spike protein | 100% | 44 | 0 | the first spans columns 7–22 | 44 |
| MERS-CoV spike gene | 100% | 10 | 1 | columns 3518–3572 (55 nt): min support 2, 29 low-support positions, bound 15.8% | 9 |
| HIV-1 *pol* protein | 100% | 1 | 1 | columns 373–381: support 3, bound 29.2% | 0 |
| HIV-1 *pol* DNA | 70% | 2 | 0 | columns 3046–3077 (32 nt, min incidence 80.1%, bound 78.5%) and 3156–3185 (30 nt, 71.5%, bound 69.8%) | the same 2 |

The HIV-1 *pol* DNA regions are compared with the paper in
[Comparison with the papers](/explanation/paper-comparison/).

## Computing regions

Regions are computed from the results, so they can be recomputed at another threshold without
reanalysing the alignment: `dima hcs -i results.json --hcs-min-incidence 95`. During an analysis,
`--hcs-output`, `--tsv-dir` and `--table hcs` write them. See
[Find conserved sequences](/how-to/conserved-sequences/).

## References

[^ref]
