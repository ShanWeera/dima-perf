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
(`min_incidence`) and the lowest member support (`min_support`).

## Flagging regions that rest on few sequences

The paper, like DiMA v5.0.9, lets any position qualify, whatever its support. A k-mer carried by
2 of 2 sequences is "100% conserved", but that says little. DiMA keeps every qualifying position
and **flags** the regions that include positions with support at or below $T$, as the DiMA
lineage did: Abd Raman et al. list such positions in their conserved-sequence tables, marked with
an asterisk ("Positions with total number of sequences less than 100") [@raman2020], and Hu et
al. show them "to maintain the protein sequence structure" [@hu2013]. Each region lists
`low_support_positions` — every position in its span with support $\le T$, member or not — and is
`flagged` when the list is not empty. Flagged regions raise the `hcs_low_support_regions`
warning.

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

A region's `conservation_lower_bound_95` is the lowest bound among its members. The interval is
conservative — its coverage is at least the nominal 95% [@brown2001] — so a high bound is
strong evidence. For a k-mer carried by every sequence:

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
