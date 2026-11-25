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
[the command line](/reference/cli/)). The default support threshold is now the paper's 100
(5.x used 30). Some results differ by design; each difference is listed with its reason in
[Differences from DiMA v5.0.9](/explanation/differences/).

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

A position needs more sequences with a valid k-mer than the support threshold (`-s`, default 100)
for the paper's corrected entropy and to enter the summary statistics. With smaller alignments,
lower `-s` and report the value you used, or choose `--estimator chao-wang-jost`, which corrects
every position with at least two sequences [@chao2013]. See
[Choose the support threshold](/how-to/analyse/#choose-the-support-threshold) and
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
`--estimator chao-wang-jost`. See [Entropy and its correction](/explanation/entropy/).

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

- It was published in 2025, and its source study applied it only to nucleotide sequences of
  viral populations within one host. Using it on protein k-mers, or on sequences from many hosts
  as in the MERS-CoV and HIV-1 examples of this documentation, is DiMA's extension.
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
floating-point program, DiMA rounds. Its values are accurate to about 14 significant digits on
alignments of thousands of sequences, and to at least 10 at 17.5 million sequences per
position (the opt-in Chao–Wang–Jost estimator to about 15 digits at any size). The error is
at least $10^{10}$ times smaller than the sampling error of the estimate. [Numerical accuracy](/explanation/numerical-accuracy/)
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
