---
title: How DiMA measures diversity
description: The concepts behind every DiMA result — k-mer positions, support, the support threshold and its labels — with a worked example from the paper's MERS-CoV dataset.
---

DiMA (*sequence Diversity dynamics analyser for viruses*) describes how variable a viral protein
or gene is along its length [@tharanga2025]. It continues a line of work that measured the
diversity of dengue, HIV-1 and influenza A proteomes position by position
[@khan2008; @hu2013; @raman2020]. This page introduces the quantities every result is built
from; the following pages derive each measure in detail.

## The input: an alignment

DiMA reads a **multiple sequence alignment**: $N_\text{seq}$ sequences of the same length $L$
(the alignment columns), protein or nucleotide, in FASTA format. Column $j$ of every sequence
holds the residue that the aligner placed at that column, or a gap. How DiMA reads the file —
case, gaps, ambiguity codes, white space, compression — is described in
[Reading alignments](/explanation/input/).

## k-mer positions

DiMA slides a window of $k$ columns along the alignment, one column at a time. The window that
starts at column $p$ covers columns $p, p+1, \dots, p+k-1$, so there are

$$
P = L - k + 1
$$

**positions**, numbered from 1. In each sequence, the $k$ characters inside the window form that
sequence's **k-mer** at position $p$.

The default $k$ follows the alphabet: 9 for proteins, the length of the peptides presented by HLA
class I molecules [@falk1991; @rammensee1993; @tharanga2025], and 27 for nucleotides, the codons
of a 9-mer. Any $k$ from 1 to $L$ can be chosen with `-k`.

## Valid k-mers and support

A k-mer is **valid** when all its $k$ characters are residues. A gap, a missing-data symbol, an
ambiguity code, a stop or frameshift symbol (see the [symbol tables](/reference/symbols/)) anywhere
in the window makes the k-mer invalid, and that sequence does not contribute at that position.

The **support** $N$ of a position is the number of sequences with a valid k-mer there. The
distinct valid k-mers are the position's **variants**; if variant $i$ occurs $c_i$ times, then
$c_1 + c_2 + \dots + c_D = N$, where $D$ is the number of distinct variants. Every other measure
of a position is computed from these counts alone, except the optional similarity-weighted
entropy, which also compares the variants residue by residue.

## The support threshold and the low-support labels

Estimates from few sequences are unreliable, so DiMA compares each position's support with a
**support threshold** $T$ (`-s`). Its default is the paper's, 100 ("default is 100 sequences,
arbitrarily defined"), which the paper also uses for HIV-1 [@tharanga2025]; the paper's Fig. 2
shows 30, the default of DiMA v5.0.9:

| Label | Support | Meaning |
|---|---|---|
| NS | $N = 0$ | no support: no sequence has a valid k-mer |
| LS | $0 < N < T$ | low support |
| ELS | $N = T$ | support equal to the threshold |
| eligible (no label) | $N > T$ | enough support |

Only **eligible** positions receive the paper's sample-size correction of the entropy, and only
they enter the summary statistics (mean, lowest and highest entropy). LS and ELS positions are
still reported, with their observed values; NS positions have no entropy at all, because there is
nothing to measure — where the paper assigns them a pseudo-entropy of 0 (see
[why DiMA departs from it](/explanation/entropy/#positions-without-support)). The
`low_support_count` of a run is the number of NS, LS and ELS positions, as in DiMA v5.0.9.

## What is measured at each position

- **Shannon entropy** of the variant distribution, corrected for sample size, and the observed
  (plug-in) value; the estimated **sample coverage**
  ([Entropy and its correction](/explanation/entropy/)).
- Optionally (`--similarity-weighted-entropy`), the **similarity-weighted entropy**, in which
  variants that differ at few residues count for less, and the observed and similarity-weighted
  entropies divided by $\log_2 D$, which puts them between 0 and 1: an addition to the paper's
  method [@wu2025]
  ([Similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in)).
- **Diversity motifs**: each variant is classified as Index, Major, Minor or Unique, with its
  incidence; the incidence of all non-Index (total) variants and of the distinct non-Index
  variants ([Diversity motifs and summaries](/explanation/motifs/)).
- Optionally, the count of every variant per value of chosen FASTA header fields such as host,
  country or year ([Stratify by header metadata](/how-to/metadata/)).
- Across positions: **historically conserved sequences**, stretches whose most common k-mer is
  (almost) invariant ([Historically conserved sequences](/explanation/conserved-sequences/)).

## A worked example: MERS-CoV spike position 25

The paper's Fig. 1C dataset ships with DiMA as `samples/mers_spike_aa.fasta`: 191 MERS-CoV spike
protein sequences, 1,353 columns, so $P = 1{,}353 - 9 + 1 = 1{,}345$ positions of 9-mers.
Position 25 covers columns 25–33:

| Variant | Count | Incidence | Motif |
|---|---|---|---|
| `SVKSACIEV` | 129 | 75.88% | Index |
| `SFKSACIEV` | 14 | 8.24% | Major |
| `SVKPACIEV` | 9 | 5.29% | Minor |
| `SAKSACIEV` | 6 | 3.53% | Minor |
| `SLKSACIEV` | 4 | 2.35% | Minor |
| `SVKSACIAV` | 4 | 2.35% | Minor |
| `SAKSVCIEV`, `SIKSACIEV`, `SVKSACIKV`, `SVKSVCIEV` | 1 each | 0.59% each | Unique |

The support is $N = 170$: 21 of the 191 sequences have a gap or an ambiguity code somewhere in
columns 25–33. With the default $T = 100$ the position is eligible. Its observed entropy is
1.4224 bits and its corrected entropy 1.4617 bits; 41 sequences (24.12%) carry a non-Index
variant, and those 41 sequences carry 9 distinct variants (21.95%, the paper's 22.0%). The
following pages show how each of these numbers is obtained.

## Determinism

Every quantity above is computed exactly from the variants and their counts: DiMA uses no random
numbers, so the same alignment and options always give the same results, on every operating
system and with any number of threads (see [Make results reproducible](/how-to/reproducibility/)).
Where the paper's procedure is random — the entropy correction — DiMA computes the exact expected
value of its regression intercept instead of one random draw (see
[The exact expectation](/explanation/entropy/#the-exact-expectation)).

## References

[^ref]
