---
title: Frequently asked questions
description: Short answers to the questions newcomers ask most — what DiMA is, what it needs, what it does with your data, how to plot, cite and update, and where to get help.
---

Each answer is short and links to the page that explains it in full. Error messages and warnings
are explained in [Troubleshooting](/how-to/troubleshooting/), and the terms used in the results
in the [Glossary](/reference/glossary/).

## Before you start

### Is this the DiMA of the paper?

Yes. `dima` implements the method of the DiMA paper [@tharanga2025] and reproduces its published
results ([Comparison with the papers](/explanation/paper-comparison/)). It replaces `dima-cli` 5.x,
which stays installable from PyPI but is no longer developed, and the web server.

### I used dima-cli 5.x. What changed?

The analysis is now a subcommand: `dima analyze -i alignment.fasta`. Several options were renamed,
and short options now have the meanings common to command-line tools: `-t` sets the number of
threads, `-q` and `-v` how much is printed, and `-k` the k-mer length (see
[the command line](/reference/cli/)). The options of `dima-cli` 5.0.9 and their equivalents:

| `dima-cli` 5.0.9 | `dima` | Note |
|---|---|---|
| `dima-cli -i FILE -o OUT` | `dima analyze -i FILE -o OUT.json` | Without `-o`, the results go to standard output |
| `-t json` (`--type`) | `-O json` (`--output-format`) | Also `jsonl` and `tsv`; the file extension usually says it. `-t` is now the number of threads |
| `-l K` (`--length`) | `-k K` (`--kmer-length`) | Default 9 for proteins and 27 for nucleotides (5.0.9: 9 for both) |
| `-s T` (`--sthresh`) | `-s T` (`--support-threshold`) | Default 100, the paper's (5.0.9: 30) |
| `-a protein`, `-a nucleotide` | `-a protein`, `-a dna` or `-a rna` (`nucleotide` is accepted for `dna`) | Default: detected from the data (5.0.9: `protein`) |
| `-f FIELDS` (`--format`) | `--header-format FIELDS` | `--metadata-fields` then chooses which fields to stratify by (default: all) |
| `-n VALUE` (`--fillna`) | `--header-fillna VALUE` | Default `Unknown` |
| `-q NAME` (`--query`) | `--query-name NAME` | Default: the file name (5.0.9: `Unknown Query`). `-q` is now quiet |
| `-c FILE` (`--hcsout`) | `--hcs-output FILE` | `.json` or `.tsv` |
| `-e PERCENT` (`--htresh`) | `--hcs-min-incidence PERCENT` | Above 50 and at most 100, compared exactly |
| `-v` (`--version`) | `-V` (`--version`) | `-v` is now verbose |

To run with 5.0.9's defaults, give its k and threshold explicitly — `-s 30`, and `-k 9` for a
nucleotide alignment:

```sh
dima analyze -i spike_nt.fasta -a dna -k 9 -s 30 -o spike_nt.json
```

Even then, some results differ by design: the entropy correction is computed exactly instead of
from random subsamples, positions without support have no entropy (`null`) instead of 0, and
`average_entropy` is the mean of the eligible positions only (v5.0.9's mean over every position
is `all_positions_summary.average_entropy`). [Compare with DiMA v5.0.9](/how-to/reproducibility/#compare-with-dima-v509)
shows how to compare the keys, and [Differences from DiMA v5.0.9](/explanation/differences/)
lists each difference with its reason.

### Does my data leave my computer?

No. The command-line tool and the desktop app read and write local files only and make no network
connections. The desktop app opens a web page, in your browser, only when you click a link.

### Is DiMA only for viruses?

DiMA was designed and validated on viral alignments. The computation works on any protein or
nucleotide alignment, but its defaults were chosen for viral data: k = 9 for proteins is the
length of the peptides presented by HLA class I molecules [@falk1991; @rammensee1993;
@tharanga2025] (27 for nucleotides), and the paper set the support threshold of 100
arbitrarily [@tharanga2025]. Check them before using DiMA on other organisms
([Choose the k-mer length](/how-to/analyse/#choose-the-k-mer-length),
[Choose the support threshold](/how-to/analyse/#choose-the-support-threshold)).

### What input does DiMA need?

A multiple sequence alignment in FASTA format, protein or nucleotide, plain or compressed (gzip,
bzip2 or Zstandard; decompress an xz file first, or recompress it with Zstandard). DiMA does not
align sequences itself. Convert other alignment formats
first, for example Stockholm with Easel's `esl-reformat afa alignment.sto > alignment.fasta` or
with Biopython's `AlignIO.convert` [@cock2009]. See [Reading alignments](/explanation/input/).

### How many sequences do I need?

Two numbers matter, the support threshold $T$ (`-s`, default 100) and a position's support $N$,
the sequences with a valid k-mer there:

- A position enters the summary statistics only with support **above** $T$, whatever the
  estimator. With 26 sequences and the default $T = 100$, no position is eligible and the
  summaries are empty, even with `--estimator chao-wang-jost`.
- The paper's correction, the default estimator, needs at least five points (a minimum taken
  from DiMA v5.0.9, which the paper does not state), so it corrects a position only when its
  support is at least $1.25\,T$. For positions that all $N$ sequences support to be corrected,
  choose $T$ at most $0.8\,N$: $T = 20$ for 26 sequences.

So with a small alignment, lower `-s` and report the value you used. The alternative estimators
`--estimator chao-wang-jost` [@chao2013] and `--estimator miller-madow` [@miller1955] estimate
the entropy of every position with at least two sequences, low-support ones included, but those
positions still stay out of the summaries. See
[Choose the support threshold](/how-to/analyse/#choose-the-support-threshold), which shows how to
check which positions were corrected, and
[Alternative estimators](/explanation/entropy/#alternative-estimators).

### Which version of this documentation should I read?

The one that matches `dima --version`; choose it in the version menu at the top of the page.
`latest` describes the development version (the `master` branch) and may include changes that are not released yet. In
released builds, the documentation link printed by `dima --help` and shown in the desktop app
leads to the documentation of that version; builds from source link `latest`.

## Using DiMA

### How do I get the entropy per host, country or year?

Split the alignment by that header field, for example with `seqkit grep` [@shen2016], and analyse
each part. `--metadata-fields` does something else: it counts each variant's sequences per value
of a header field ([Stratify by header metadata](/how-to/metadata/)). Before comparing the parts,
see the next question.

### Can I compare diversity between proteins or datasets?

Yes, that is what DiMA is for [@tharanga2025], but only between runs with the same k, alphabet and
support threshold, and between positions of similar support: the scale of entropy depends on the
window length and the alphabet, and entropy estimates grow with the number of sequences
[@miller1955; @chao2013], a dependence the paper's correction reduces but does not remove. Report
the support and the sample coverage of both analyses, and check that the conclusion holds with
`--estimator chao-wang-jost`. [Compare two alignments](/how-to/analyse/#compare-two-alignments)
shows which numbers to set side by side — the eligible mean, the share of the alignment it
covers, and the support behind it — with a worked example. See also
[Entropy and its correction](/explanation/entropy/).

The DiMA lineage compares proteins by their mean entropy over eligible positions, by their
highest entropy, and by the share of positions that are highly conserved (Index incidence of 90%
or more), mixed-variable or highly diverse (20% or less) [@tharanga2025, Results; @hu2013,
Results and Fig. 3]. It reports no statistical test of a difference, and neither does DiMA.
Datasets of very different size can still rank the wrong way round [@zhao2019, Section 3.1]:
[Read the comparison](/how-to/analyse/#read-the-comparison) gives three ways to check, from
comparing at similar sample coverage to subsampling the larger dataset.

The optional similarity-weighted and normalised entropies need the same care, and they are not
corrected for sample size at all: the observed entropy is lower on average with fewer sequences
[@zhao2019, Section 3.1], the similarity-weighted entropy also changes with the number of
sequences, and the normalised entropy also depends on sample size
[@gregori2014, Sections 2.4 and 3.2]. Compare them only between runs with the same k and
alphabet, and between positions of similar support.

### What is similarity-weighted entropy and should I use it?

It is an optional measure from Wu [@wu2025, Section 2.1], not part of the DiMA paper's method.
Shannon entropy counts a variant that differs from the others at one residue as just as different
as an unrelated one. Similarity-weighted entropy gives each variant less weight the more similar
it is to the rest of the position, so near-identical variants count for less, and it is never
more than the observed entropy. DiMA computes it, with both entropies divided by $\log_2$ of the
number of distinct k-mers (which puts them between 0 and 1), only when you ask:
`--similarity-weighted-entropy`, or **Compute similarity-weighted entropy (Wu 2025)** under
**Advanced** in the desktop app.

It can help when it matters how different the variants are, not only how many there are and how
common. Use it next to the entropy, not instead of it, and with care:

- It was published in 2025, and its source study applied it only to nucleotide sequences:
  simulated populations, a viroid mutant pool and the populations it gave within single plants,
  and one plant-virus dataset. Using it on protein k-mers, or on sequences from many hosts as in
  the MERS-CoV and HIV-1 examples of this documentation, is DiMA's extension.
- None of its values is corrected for sample size: they change with the number of sequences, so
  compare them only between positions and datasets of similar support (see the previous
  question).

[Add similarity-weighted entropy](/how-to/analyse/#add-similarity-weighted-entropy-optional)
shows an example, and
[Similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) gives
the formulas.

### How do I plot entropy along the alignment?

In the desktop app, export the entropy chart as PNG or SVG. From the command line, plot `entropy`
against `position` from the positions table; positions without support have no entropy (`NA`)
and should stay gaps, not zeros. See
[Plot entropy along the alignment](/how-to/outputs/#plot-entropy-along-the-alignment).

### Can I run DiMA from Python or R?

Yes, as a command, then read its tables or JSON
([Read the tables in R and Python](/how-to/outputs/#read-the-tables-in-r-and-python)). In Python:

```python
import subprocess
subprocess.run(["dima", "analyze", "-i", "spike.fasta", "--tsv-dir", "tables"], check=True)
```

In R:

```r
stopifnot(system2("dima", c("analyze", "-i", "spike.fasta", "--tsv-dir", "tables")) == 0)
```

There is no Python or R package. Rust programs can use the `dima_core` library, the one both the
command-line tool and the desktop app are built on.

## Results and versions

### What is the eligible mean?

The average entropy of the eligible positions, the positions with support above the support
threshold $T$, each position counted once. It is the run's mean entropy: `average_entropy` in the
results, `entropy mean` in the line `dima analyze` prints at the end, the *mean* chip of the
desktop app and the dashed grey line labelled *eligible mean* on the app's entropy chart. For the
MERS-CoV spike sample (`samples/mers_spike_aa.fasta` in the repository) it is 0.1261 bits. Positions with support at or below $T$ are not in
it, so it is not the mean of every point the chart draws. When the chart shows
similarity-weighted entropy, the same line is the eligible mean of similarity-weighted entropy
(0.0363 bits for the same sample). See
[the eligible mean](/explanation/motifs/#the-eligible-mean) for the formula and a worked example,
and [Read the eligible mean line](/how-to/desktop-app/#read-the-eligible-mean-line) for the chart.

### Why do my results differ from the paper or from dima-cli 5.x?

[Comparison with the papers](/explanation/paper-comparison/) reproduces the paper's results, and
[Differences from DiMA v5.0.9](/explanation/differences/) lists every deliberate difference from
5.x with its reason. For example, the default support threshold is 100, as in the paper, where
5.x used 30.

### Will updating DiMA change my results?

Only where the [changelog](/reference/changelog/) says so under "Methodology". Each version gives
byte-identical results for the same input and options on every platform and with any number of
threads, and every result file records the version that produced it
([Comparing versions](/how-to/reproducibility/#comparing-versions)).

### How exact are the numbers?

Exact to far more digits than any analysis needs, but not to the last digit. Like any
floating-point program, DiMA rounds. On the test datasets in the repository (up to 3,756
sequences), the rounding error is in the last one or two of the 17 significant digits written to
the results, and at 17.5 million sequences per position it reaches at most the 11th digit (the
opt-in Chao–Wang–Jost estimator stays within a few units in the last place at any size; the
opt-in similarity-weighted entropy reaches at most the 10th digit). The error is at least $10^{10}$ times smaller than
the sampling error of the estimate ($10^9$ times for similarity-weighted entropy). [Numerical accuracy](/explanation/numerical-accuracy/)
has the measurements.

### How long will an analysis take, and how much memory does it need?

The benchmark report linked from [Benchmarks](/explanation/benchmarks/) gives measured times and
memory for the paper's datasets and larger alignments; memory grows with the diversity of the alignment, not with the number of
sequences ([Large alignments](/how-to/large-alignments/)). With `-v`, DiMA prints the time of
each phase and the peak memory at the end of a run.

### Can I use DiMA on a cluster, or in Snakemake or Nextflow?

Yes: [Run on clusters and in pipelines](/how-to/hpc/) covers batch schedulers, thread counts,
exit codes and workflow managers. A Bioconda recipe will be submitted
([Install DiMA](/getting-started/install/#bioconda-and-containers)); there is no Galaxy tool yet.

## Help

### Where can I get help or report a bug?

Ask questions in [GitHub Discussions](https://github.com/ShanWeera/dima-perf/discussions) and
report bugs in [GitHub Issues](https://github.com/ShanWeera/dima-perf/issues/new/choose); search
both first, as your question may already have an answer. The bug form asks for the version, the
command and, if you can share it, the run report (`--run-report`), which records how the run went
without user or host names.

### How do I cite DiMA?

Cite the paper and the version you used: `dima cite` prints both, and
`dima cite --from results.json` prints a methods paragraph and every reference of that run. See
[Citing DiMA](/reference/citing/).

### What licence does DiMA have?

The MIT licence. `dima --licenses` prints the licences of DiMA and of the components built into
it.

## References

[^ref]
