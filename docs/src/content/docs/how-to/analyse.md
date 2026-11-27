---
title: Analyse an alignment
description: Run dima analyze on a protein or nucleotide alignment — prepare the input, choose k, the support threshold, the alphabet and the estimator, optionally add similarity-weighted entropy, read the results and compare two alignments.
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
dima analyze -i <(seqkit grep -n -r -p 'Homo sapiens' all.fasta) --query-name human -o human.json
```

`seqkit` [@shen2016] is used here only as an example of a tool that writes FASTA to a pipe.
Standard input, `/dev/stdin` and process substitution (`<(...)`) have no file name, so the
[query name](/reference/glossary/#query-name) of their results is `Unknown Protein` (or
`Unknown DNA`, `Unknown RNA`) unless you give one with `--query-name`, as above; it names the
results in tables and figures.

`-i -` also reads standard input. If nothing is piped in — standard input is your terminal —
DiMA waits for you to type or paste an alignment and to end it with Ctrl-D (Ctrl-Z, then Enter,
on Windows). Without `-i`, it does not wait: it stops at once with a usage error (exit 2):

```text
error: no input alignment
  hint: give one with -i FILE, or pipe it in: dima analyze < alignment.fasta
```

## Run it

```sh
dima analyze -i spike.fasta -o spike.json
```

Without `-o` (and without `--tsv-dir` or `--hcs-output`), the results go to standard output as
JSON. DiMA prints warnings and then the [summary line](/reference/glossary/#summary-line) of the
run on standard error; add `-v` for the thread count, the files written, the time of each phase
and the memory used ([Read the -v report](/how-to/large-alignments/#read-the--v-report)), or
`-q` for errors only. For the MERS-CoV spike sample (`samples/mers_spike_aa.fasta` in the
repository; [Your first analysis](/getting-started/first-analysis/#1-get-the-alignment) shows
how to download it) the summary line reads:

```text
analysed 191 sequences, 1345 positions (1345 eligible) in 0.01 s; entropy mean 0.1261, min 0.0000, max 1.5878 bits
```

Its parts, separated by `; `, are:

| Part | When it appears | What it says |
|---|---|---|
| `analysed N sequences, P positions (E eligible) in T` | Always | The sequences, the [positions](/reference/glossary/#position) (one per k-mer start), how many are [eligible](/reference/glossary/#eligible-position) (support above the support threshold), and the run time |
| `entropy mean M, min L, max H bits` | When some position is eligible | The [eligible mean](/reference/glossary/#eligible-mean) (`average_entropy` in the results: the average entropy of the eligible positions, each counted once), and the lowest and highest entropy among them |
| `no entropy summary (no eligible positions)` | Instead, when no position is eligible | The support threshold is too high for these data (see [Choose the support threshold](#choose-the-support-threshold)) |
| `similarity-weighted mean S bits (observed O)` | With `--similarity-weighted-entropy`, when some position is eligible | See [Add similarity-weighted entropy](#add-similarity-weighted-entropy-optional) |
| `C conserved regions` | When conserved sequences are written (`--hcs-output`, `--tsv-dir` or `-o … --table hcs`) | The regions at the `--hcs-min-incidence` threshold, [flagged](/reference/glossary/#flagged-region) ones included |
| `W warnings (see above)` | When warnings were printed | How many: those about the data and those about the options; notes (`note[…]`, printed with `-v`) are not counted |

Entropies have four decimals; a value that is not zero never reads `0.0000` there, but
`< 0.0001`. The results keep every digit. Two examples with the optional parts, the first with
conserved sequences at 95%, the second with 26 sequences and the default threshold of 100:

```text
analysed 191 sequences, 1345 positions (1345 eligible) in 0.02 s; entropy mean 0.1261, min 0.0000, max 1.5878 bits; 11 conserved regions
analysed 26 sequences, 1267 positions (0 eligible) in 0.01 s; no entropy summary (no eligible positions); 1 warning (see above)
```

The summary line is the last line of the run, after any `-v` report, so it is the last line of a
log. When standard error is a terminal (not with `-q`), DiMA adds a reminder of how to cite the
run:

```text
Cite: Tharanga et al. 2025, doi:10.1093/bib/bbae607 (`dima cite --from spike.json` lists this run's references)
```

It names the results file only when the results are a regular file (quoted for the shell when its
name needs it); otherwise, for results on standard output for example, it reads
``(`dima cite` prints the references)``. See [Citing DiMA](/reference/citing/).

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

A position's [support](/reference/glossary/#support) is the number of sequences with a
[valid k-mer](/reference/glossary/#valid-k-mer) there, one without a gap or an ambiguity code.
`-s`/`--support-threshold` sets the [support threshold](/reference/glossary/#support-threshold)
$T$. Positions with support above $T$ are **eligible**: they enter the summary statistics. The
default, 100, is the paper's value, which it describes as "arbitrarily defined"
[@tharanga2025]:

```sh
dima analyze -i spike.fasta -s 50 -o spike.T50.json
```

With the paper's estimator (`dima`, the default), an eligible position's entropy is
[corrected](/reference/glossary/#corrected-entropy) for sample size *where possible*. The
correction needs at least five points (the full data set included; a minimum taken from DiMA
v5.0.9, which the paper does not state), so a position is corrected only when its support is at
least $1.25\,T$ (125 sequences for $T = 100$). With support between $T$ and $1.25\,T$ — and in the
one tiny case $T = 2$, $N = 3$ — it keeps its [observed entropy](/reference/glossary/#observed-entropy);
such positions are still eligible and are included in the summaries. `entropy_fallback` records
why ([Entropy fallback](/reference/glossary/#entropy-fallback)), and
[Entropy and its correction](/explanation/entropy/) explains the rule.

So for a small dataset of $N$ sequences:

- the summaries need positions with support **above** $T$, whatever the estimator, so $T$ must be
  below $N$; if it is not, no position is eligible and DiMA warns
  (`threshold_exceeds_sequence_count`; see [Troubleshooting](/how-to/troubleshooting/#no-position-is-eligible));
- for the default estimator to correct the positions that all $N$ sequences support, $T$ must be
  at most $0.8\,N$.

This command counts the eligible positions by how their entropy was obtained — `corrected`, or
the reason it was not:

```sh
jq -c '[.results[] | select(.low_support == null) | .entropy_fallback // "corrected"]
       | group_by(.) | map({(.[0]): length}) | add' results.json
```

For example, the 26-sequence alignment of DiMA v5.0.9's own tests, from the repository:

```sh
curl -fLO https://raw.githubusercontent.com/ShanWeera/dima-perf/master/tests/data/dima-v5.0.9/input.afa.bz2
dima analyze -i input.afa.bz2 -s 25 -o small.s25.json
dima analyze -i input.afa.bz2 -s 20 -o small.s20.json
```

With $T = 25$, the 997 eligible positions keep their observed entropy — support 26 is below
$1.25 \times 25 = 31.25$ — and the command prints `{"too_few_points":997}`; with $T = 20$, at
most $0.8 \times 26 = 20.8$, all 1,031 eligible positions are corrected: `{"corrected":1031}`.

There is no universally right value. A lower $T$ admits positions with fewer sequences, whose
entropy rests on less data; a higher $T$ leaves more positions labelled low support. Whatever
you choose, report it: it is recorded in the results and printed by `dima cite --from`. See
[the support threshold](/explanation/method/#the-support-threshold-and-the-low-support-labels).

## Set the alphabet if needed

By default DiMA detects protein, DNA or RNA from the residues of the first records, with the
rules of Easel's `esl_abc_GuessAlphabet`, the sequence library of HMMER [@eddy2011]. When the
rules do not decide, DiMA stops and asks for the alphabet. That happens for three reasons:

- too few residues: at most 10 letters in the first records;
- both T and U present, so DNA and RNA cannot be told apart;
- a composition that is neither clearly nucleotide (A, C, G and T or U, all four seen, with at
  most 2% other letters) nor clearly protein (any of the letters E, F, I, J, L, O, P, Q and Z,
  which no nucleotide alphabet uses, or a wide mix of protein letters).

The message names the reason, for example
`the residue composition of the first 4 records (420 letters) does not identify the alphabet`.
Give the alphabet:

```sh
dima analyze -i short_peptides.fasta -a protein -o peptides.json
```

`-a` accepts `auto`, `protein`, `dna` and `rna`. If you set it and the data look different,
DiMA uses your choice; in `--validation permissive` it also warns (`alphabet_contradiction`). If
the choice is wrong in the default strict mode, the first residue of the other alphabet stops
the run, for example
`character 'F' (0x46) is not valid in a dna alignment` — check `-a` first; see
[Choose the validation mode](/how-to/validation/#a-wrong---alphabet).

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
| `miller-madow` | The classical first-order correction [@miller1955]; also corrects positions with support ≤ $T$ |
| `raw` | The observed entropy, uncorrected |

`chao-wang-jost` and `miller-madow` estimate the entropy of every position supported by at least
two sequences, the low-support (LS and ELS) ones included; with `dima` and `raw` those keep
their observed entropy. Whatever the estimator, low-support positions stay out of the summaries.
In the 26-sequence example above with `-s 25`, both give their estimate at all 260 LS and ELS
positions. Every position also reports its observed entropy and its sample coverage, whatever the
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

The summary line gains the mean similarity-weighted entropy and, in brackets, the mean observed
entropy of the same positions:

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

## Compare two alignments

To compare the diversity of two proteins, genes or datasets, analyse each with the **same k,
alphabet, support threshold and estimator** — every number below depends on them — and keep
each result in its own file. This example compares the MERS-CoV spike sample with the HIV-1
*pol* protein alignment of the repository's test data:

```sh
curl -fLO https://raw.githubusercontent.com/ShanWeera/dima-perf/master/samples/mers_spike_aa.fasta
curl -fLO https://raw.githubusercontent.com/ShanWeera/dima-perf/master/tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2
dima analyze -i mers_spike_aa.fasta -o mers.json
dima analyze -i HIV1_ALL_2020_pol_PRO.fasta.bz2 -o hiv.json
```

First check that the settings agree, and read the summary of each run:

```sh
jq '{query_name, positions: .position_count, eligible: .eligible_position_count,
     average_entropy, k: .kmer_length, T: .support_threshold,
     alphabet: .run.parameters.alphabet.value, estimator: .run.parameters.estimator.value}' mers.json hiv.json
```

```json
{
  "query_name": "mers_spike_aa",
  "positions": 1345,
  "eligible": 1345,
  "average_entropy": 0.12613379461699337,
  "k": 9,
  "T": 100,
  "alphabet": "protein",
  "estimator": "dima"
}
{
  "query_name": "HIV1_ALL_2020_pol_PRO",
  "positions": 1189,
  "eligible": 476,
  "average_entropy": 1.9072170695305426,
  "k": 9,
  "T": 100,
  "alphabet": "protein",
  "estimator": "dima"
}
```

Then read three things side by side:

1. **The eligible mean with the lowest and highest entropy** (`average_entropy`,
   `lowest_entropy`, `highest_entropy`), the numbers the paper reports for each protein
   [@tharanga2025]: here 0.1261 bits for MERS-CoV spike and 1.9072 bits for HIV-1 *pol*.
2. **The eligible fraction**, `eligible_position_count / position_count`: the share of the
   alignment the summary describes. For MERS-CoV spike it is 1,345 of 1,345 positions (100%);
   for HIV-1 *pol* only 476 of 1,189 (40.0%). A mean over 40% of an alignment describes those
   positions, not the whole protein; `support_label_counts` says how the others are labelled.
3. **The support of the eligible positions**, which tells how many sequences each value rests
   on:

```sh
jq '[.results[] | select(.low_support == null) | .support]
     | {min: min, max: max, mean: (add / length | round)}' mers.json hiv.json
```

```json
{
  "min": 168,
  "max": 181,
  "mean": 174
}
{
  "min": 105,
  "max": 3746,
  "mean": 3632
}
```

The MERS-CoV values rest on 168 to 181 sequences each; the HIV-1 values on 105 to 3,746. Two
eligible HIV-1 positions (support 105, below $1.25\,T = 125$) keep their observed entropy, as
the `entropy_fallback` count of [Choose the support threshold](#choose-the-support-threshold)
shows (`{"corrected":474,"too_few_points":2}`).

<!-- PHASE0: research-backed guidance on reading the comparison goes here: how the paper and its
lineage compare diversity between proteins or datasets (tharanga2025, hu2013, khan2008), what a
difference in entropy means in size (effective number 2^H: Leinster & Cobbold 2012, Jost 2006 or
Hill 1973), and how to compare datasets of different size (rarefaction or coverage-based
subsampling: Gregori 2014 §2.5/3.2, chao2012; Chao et al. 2014). Add only statements read in
full, with their locators. -->

## Speed

DiMA uses all the CPUs it may use (on a batch scheduler, the CPUs of your allocation); `-t N`
sets the number. The results never depend on it. See
[Run on clusters and in pipelines](/how-to/hpc/) and [Analyse large alignments](/how-to/large-alignments/).

## References

[^ref]
