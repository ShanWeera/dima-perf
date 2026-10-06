---
title: Analyse an alignment
description: Run dima analyze on a protein or nucleotide alignment — prepare the input, choose k, the support threshold, the alphabet and the estimator, and read the results.
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
milestones and a performance report, or `-q` for errors only.

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

## Read the results

The summary keys at the top of the JSON answer the paper's questions directly:

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
