---
title: Analyse an alignment
description: Run dima analyze on a protein or nucleotide alignment — prepare the input, choose k, the support threshold, the alphabet and the estimator, optionally add similarity-weighted entropy, and read the results.
---

This guide covers the decisions of a typical analysis. Every option is listed in the
[command-line reference](/reference/cli/#dima-analyze).

## Prepare the input

DiMA analyses an existing **multiple sequence alignment** in FASTA format: every sequence must
have the same number of columns. Align your sequences first (for example with MAFFT or Clustal
Omega [@katoh2013; @sievers2011]), and keep them protein or nucleotide, not mixed.

The file can be wrapped or not, upper or lower case, with Unix or Windows line endings, plain or
compressed with gzip, bzip2 or Zstandard — DiMA recognises compression by content, so the
file name does not matter. It can also come from a pipe:

```sh
dima analyze -i spike.fasta.zst -o spike.json
zcat spike.fasta.gz | dima analyze -o spike.json            # standard input
dima analyze -i <(seqkit grep -n -r -p 'Homo sapiens' all.fasta) -o human.json
```

`seqkit` [@shen2016] is used here only as an example of a tool that writes FASTA to a pipe.

## Run it

```sh
dima analyze -i spike.fasta -o spike.json
```

Without `-o` (and without `--tsv-dir` or `--hcs-output`), the results go to standard output as
JSON. DiMA prints warnings and a one-line summary on standard error; add `-v` for progress
milestones and a performance report, or `-q` for errors only. The summary gives entropies to four
decimals; a value that is not zero never reads `0.0000` there, but `< 0.0001`. The results keep
every digit. For the bundled MERS-CoV spike sample (`samples/mers_spike_aa.fasta`) the summary
reads:

```text
analysed 191 sequences, 1345 positions (1345 eligible) in 0.01 s; entropy mean 0.1261, min 0.0000, max 1.5878 bits
```

Its `entropy mean`, `min` and `max` cover the eligible positions only (support above the support
threshold, here all 1,345). The mean is the [eligible mean](/reference/glossary/#eligible-mean),
`average_entropy` in the results: the average entropy of the eligible positions, each counted
once.

## Choose the k-mer length

`-k` sets the window length. The default depends on the alphabet: **9** for proteins, the length
of the peptides presented by HLA class I molecules [@falk1991; @rammensee1993; @tharanga2025],
and **27** for nucleotides (the codons of a 9-mer). Any value from 1 to the alignment length is
accepted:

```sh
dima analyze -i spike.fasta -k 15 -o spike.k15.json
```

Longer k-mers capture longer stretches but are more often invalidated by a gap or an ambiguity
code, which lowers the support. The value used, and whether it was the default, is recorded in
`run.parameters.kmer_length`.

## Choose the support threshold

`-s`/`--support-threshold` sets $T$. Positions with support above $T$ are **eligible**: their
entropy is corrected for sample size (unless the correction cannot be applied there, which
`entropy_fallback` records) and they enter the summary statistics. The default, 100, is
the paper's value, which it describes as "arbitrarily defined" [@tharanga2025]:

```sh
dima analyze -i spike.fasta -s 50 -o spike.T50.json
```

There is no universally right value. A lower $T$ admits positions with fewer sequences, whose
corrected entropy rests on less data; a higher $T$ leaves more positions labelled LS. If $T$ is
not below the number of sequences, no position can be eligible and DiMA warns
(`threshold_exceeds_sequence_count`). Whatever you choose, report it: it is recorded in the
results and printed by `dima cite --from`. See
[the support threshold](/explanation/method/#the-support-threshold-and-the-low-support-labels).

## Set the alphabet if needed

By default DiMA detects protein, DNA or RNA from the residues of the first records, with the
rules of HMMER's Easel library [@eddy2011]. When they do not decide — very short sequences, or
both T and U present — DiMA stops and asks:

```sh
dima analyze -i short_peptides.fasta -a protein -o peptides.json
```

`-a` accepts `auto`, `protein`, `dna` and `rna`. If you set it and the data look different,
DiMA warns (`alphabet_contradiction`) but uses your choice.

## Choose the entropy estimator

The default, `--estimator dima`, is the paper's correction, computed exactly. Three alternatives
are available for comparison or sensitivity analyses:

```sh
dima analyze -i spike.fasta --estimator chao-wang-jost -o spike.cwj.json
```

| Estimator | When to use it |
|---|---|
| `dima` (default) | Results comparable with the paper and earlier DiMA analyses |
| `chao-wang-jost` | Lowest bias among the estimators in its authors' comparison [@chao2013]; also corrects positions with support ≤ $T$ |
| `miller-madow` | The classical first-order correction [@miller1955] |
| `raw` | The observed entropy, uncorrected |

Every position also reports its observed entropy and its sample coverage, whatever the
estimator. See [Entropy and its correction](/explanation/entropy/).

## Add similarity-weighted entropy (optional)

Shannon entropy treats two variants that differ at one residue exactly like two unrelated ones.
**Similarity-weighted entropy** [@wu2025, Section 2.1] weights each variant by how different it
is from the rest of the position, so near-identical variants count for less. It is not part of
the DiMA paper's method, and DiMA computes it only when you ask for it:

```sh
dima analyze -i spike.fasta --similarity-weighted-entropy -o spike.json
```

In the desktop app, tick **Compute similarity-weighted entropy (Wu 2025)** under **Advanced**
before you run the analysis.

Each position then has three more values; without the option they are `null` (`NA` in the
positions table). Turning it on changes no other result: the run only records the option in
`run.parameters.similarity_weighted_entropy` and its sources in `run.references`.

| Key | Meaning |
|---|---|
| `similarity_weighted_entropy` | The similarity-weighted entropy, in bits: never more than the observed entropy, and 0 at a position with a single variant |
| `normalised_observed_entropy` | The observed entropy divided by $\log_2 D$, the largest entropy possible with $D$ distinct k-mers (the [Index](/reference/glossary/#index-major-minor-and-unique) included), so between 0 and 1 |
| `normalised_similarity_weighted_entropy` | The similarity-weighted entropy divided by the same $\log_2 D$ |

The results also gain `similarity_weighted_entropy_summary`: the mean, lowest and highest
similarity-weighted entropy over the eligible positions, with the mean observed entropy of the
same positions for comparison. The values are written to the results (JSON and JSON Lines) and to
the positions table; [Choose and read outputs](/how-to/outputs/#similarity-weighted-entropy-in-the-outputs)
shows where to find them.

For example, with the MERS-CoV spike sample:

```sh
dima analyze -i mers_spike_aa.fasta --similarity-weighted-entropy -o mers.json
```

The line printed at the end of the run gains the mean similarity-weighted entropy and, in
brackets, the mean observed entropy of the same positions:

```text
analysed 191 sequences, 1345 positions (1345 eligible) in 0.01 s; entropy mean 0.1261, min 0.0000, max 1.5878 bits; similarity-weighted mean 0.0363 bits (observed 0.1207)
```

The first mean, 0.1261 bits, is that of the corrected entropy. The similarity-weighted entropy is
compared with the observed entropy instead, because it weights the observed entropy and is never
more than it. At position 25:

```sh
jq '.results[24] | {observed_entropy, similarity_weighted_entropy,
    normalised_observed_entropy, normalised_similarity_weighted_entropy}' mers.json
```

```json
{
  "observed_entropy": 1.4223886721629135,
  "similarity_weighted_entropy": 0.42754448388901667,
  "normalised_observed_entropy": 0.4281816558136978,
  "normalised_similarity_weighted_entropy": 0.12870371413126977
}
```

The position has 10 distinct 9-mers. Eight of the nine variants other than the Index differ from
it at one of their nine residues, and the ninth at two, so each is nearly identical to most of the
position and counts for little: the similarity-weighted entropy, 0.4275 bits, is 30% of the
observed entropy, 1.4224 bits. Divided by $\log_2 10 = 3.32$ bits, they become 0.4282 and 0.1287.
At a fully conserved position, such as position 7, the similarity-weighted entropy is 0 and the
two normalised values are `null`: with a single distinct k-mer, the largest entropy possible is
0, and a ratio to 0 is undefined.

Read the values with care:

- The method was published in 2025 and applied in its source study only to nucleotide sequences:
  simulated populations, a viroid mutant pool and the populations it gave within single plants,
  and one plant-virus dataset; its use on amino-acid k-mers and on sequences from many hosts is
  DiMA's extension [@wu2025].
- None of the three values is corrected for sample size: the observed entropy is lower on average
  with fewer sequences [@zhao2019, Section 3.1], the similarity-weighted entropy also changes with
  the number of sequences, and the normalised entropy also depends on sample size
  [@gregori2014, Sections 2.4 and 3.2]. Compare positions and datasets of similar support.
- All three depend on k and on the alphabet, so compare runs with the same k and alphabet.
- The similarity of two k-mers is the proportion of their residues that are the same, so every
  substitution counts alike, whatever the two residues are.

The values are computed only when an output holds them. If you write only the variants, metadata
or conserved-sequence outputs, DiMA warns and computes nothing extra:

```text
warning[flag_has_no_effect]: --similarity-weighted-entropy has no effect without results (JSON, JSONL) or the positions table
  hint: write the results (-o PATH.json) or the positions table (-o PATH.tsv --table positions, or --tsv-dir DIR)
```

The formulas and the sources are in
[Similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in).

## Read the results

The summary keys at the top of the JSON answer the paper's questions directly; `average_entropy`
is the [eligible mean](/reference/glossary/#eligible-mean):

```sh
jq '{query_name, sequence_count, position_count, eligible_position_count,
     average_entropy, lowest_entropy, highest_entropy, support_label_counts}' spike.json
```

Each entry of `results` describes one position: its support and label, entropies, incidences,
coverage and the list of variants with their motifs. For example, the five most diverse
eligible positions:

```sh
jq -c '[.results[] | select(.low_support == null)]
       | sort_by(-.entropy) | .[:5][] | {position, support, entropy}' spike.json
```

The same data as tables, ready for R or pandas, come from `--tsv-dir`:

```sh
dima analyze -i spike.fasta -o spike.json --tsv-dir tables
```

See [Choose and read outputs](/how-to/outputs/) and the [JSON reference](/reference/json/).

## Speed

DiMA uses all the CPUs it may use (on a batch scheduler, the CPUs of your allocation); `-t N`
sets the number. The results never depend on it. See
[Run on clusters and in pipelines](/how-to/hpc/) and [Analyse large alignments](/how-to/large-alignments/).

## References

[^ref]
