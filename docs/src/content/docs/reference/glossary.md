---
title: Glossary
description: The terms used in DiMA's results, options and documentation, each defined in one or two sentences with a link to the page that explains it.
---

Each term is defined briefly here and in full on the page it links to. Names in `code` are the
fields of the [JSON](/reference/json/) and [TSV](/reference/tsv/) outputs or the options of the
[command line](/reference/cli/).

## Alignment and k-mers

### Alignment column

One column of the multiple sequence alignment: the residue, or gap, that each sequence has at that
place. Columns are numbered from 1. See [the input](/explanation/method/#the-input-an-alignment).

### Position

The window of k columns that starts at a given column; a position is numbered by its first column,
so an alignment of L columns has L − k + 1 positions. See
[k-mer positions](/explanation/method/#k-mer-positions).

### k-mer and k

A k-mer is the k characters of one sequence inside a position's window. k is set with `-k`
(default 9 for proteins and 27 for nucleotides). See
[choose the k-mer length](/how-to/analyse/#choose-the-k-mer-length).

### Valid k-mer

A k-mer whose characters are all residues. A gap, missing-data symbol, ambiguity code, stop or
frameshift symbol anywhere in the window makes it invalid, and that sequence then does not count
at that position. See [valid k-mers and support](/explanation/method/#valid-k-mers-and-support).

### Variant

A distinct valid k-mer at a position, with the number of sequences that carry it.

### Distinct k-mers (D)

The number D of different valid k-mers at a position, the Index included: 1 at a fully conserved
position, 0 without support. `distinct_variants_count` is a different number: it leaves out the
Index variant (or variants, when several are tied), so at MERS-CoV spike position 25 D is 10 and
`distinct_variants_count` is 9.

## Support

### Support

The number of sequences with a valid k-mer at a position (N, `support`).

### Support threshold

The value T that each position's support is compared with (`-s`, default 100, the paper's value).
See [the support threshold](/explanation/method/#the-support-threshold-and-the-low-support-labels).

### Eligible position

A position with support above the support threshold. Only eligible positions receive the paper's
corrected entropy and enter the summary statistics.

### NS, LS and ELS

The labels (`low_support`) of positions that are not eligible: NS, no support (N = 0); LS, low
support (N below T); ELS, support equal to the threshold (N = T).

## Entropy

### Observed entropy

The Shannon entropy, in bits, of a position's variant counts as observed (`observed_entropy`). It
underestimates the entropy of the population, more so with fewer sequences. See
[observed entropy](/explanation/entropy/#observed-plug-in-entropy).

### Corrected entropy

The entropy reported as `entropy`: by default the paper's sample-size correction at eligible
positions, and the observed entropy at the other positions with support. See
[the paper's correction](/explanation/entropy/#the-papers-correction-fig-2).

### Estimator

The method behind the reported entropy (`--estimator`): `dima` (the paper's correction, the
default), `chao-wang-jost`, `miller-madow` or `raw`. Each position records the method used
(`entropy_method`). See [alternative estimators](/explanation/entropy/#alternative-estimators).

### Entropy fallback

Why a position's entropy was computed with another method than the one requested
(`entropy_fallback`), for example because its support is at or below the support threshold.

### Sample coverage

The estimated probability that one more sequence would carry a k-mer already seen at that position
(`sample_coverage`). Low coverage means that much of the diversity there has probably not been
sampled. See [sample coverage](/explanation/entropy/#sample-coverage).

### Similarity-weighted entropy

An optional entropy, in bits, computed only on request (`--similarity-weighted-entropy`;
`similarity_weighted_entropy`). Each variant's part of the observed entropy is weighted by how
different the variant is from the rest of the position, so variants that differ at only a few
residues count for less than unrelated ones. It is never more than the observed
entropy, and it is 0 at a position with a single variant. It is not part of the paper's method.
See [similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in).

### Similarity of two k-mers

The proportion of their k residues that are the same: (k − d)/k when two k-mers differ at d
residues. Every substitution counts alike, whatever the residues. Similarity-weighted entropy
uses it.

### Normalised entropy (H_n and H_nsim)

An entropy divided by log₂ D, the largest observed entropy possible with D distinct k-mers, which
puts it between 0 and 1: `normalised_observed_entropy` (H_n) for the observed entropy and
`normalised_similarity_weighted_entropy` (H_nsim) for the similarity-weighted entropy. Both are
computed only with `--similarity-weighted-entropy`, and are undefined (`null`, `NA`) at a
position with fewer than two distinct k-mers. See
[similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in).

## Diversity motifs

### Index, Major, Minor and Unique

The motif of each variant of a position: Index, the most common variant (count above 1); Major,
the next most common; Minor, the other variants seen more than once; Unique, the variants seen
once. See [motif classes](/explanation/motifs/#motif-classes).

### Incidence

A variant's count as a percentage of the position's support.

### Total variants incidence

The percentage of a position's support carried by variants other than the Index.

### Distinct variants incidence

The number of distinct non-Index variants as a percentage of the sequences that carry them. See
[incidences](/explanation/motifs/#incidences).

## Conserved sequences

### Historically conserved sequence (HCS)

A region of consecutive positions whose Index variant reaches the threshold incidence
(`--hcs-min-incidence`, default 100%). See
[historically conserved sequences](/explanation/conserved-sequences/).

### Flagged region

A conserved region that includes positions with support at or below the support threshold, so that
its conservation rests on few sequences. See
[flagging](/explanation/conserved-sequences/#flagging-regions-that-rest-on-few-sequences).

### Conservation lower bound

The exact 95% Clopper–Pearson lower bound of the Index proportion of a position or region
(`conservation_lower_bound_95`): how conserved it is at least, given the number of sequences. See
[a lower bound on conservation](/explanation/conserved-sequences/#a-lower-bound-on-conservation).

## Input

### Header format

The names of the `|`-separated fields of the FASTA headers (`--header-format`). See
[describe the headers](/how-to/metadata/#describe-the-headers).

### Metadata fields

The header fields per whose values each variant's sequences are counted (`--metadata-fields`).
See [stratify by header metadata](/how-to/metadata/).

### Fill value

The value that replaces an empty header field (`--header-fillna`, default `Unknown`).

### Validation mode

What happens to characters outside the alphabet (`--validation`): `strict` stops the run,
`permissive` excludes the k-mers that contain them, and `report` also lists every character
found. See [validate input characters](/how-to/validation/).

### Query name

The name of the analysed protein or gene in the results (`query_name`): `--query-name`, or the
input file name without its extensions. See [the query name](/explanation/input/#the-query-name).

## Results

### Diagnostic

A condition that did not stop the analysis but that you should know about, reported once with a
code, a count, examples and a hint. See [Diagnostics](/reference/diagnostics/).

### Provenance

The `run` object of the results: the version, every parameter and where it came from, the input's
checksum, what reading found, the diagnostics and the references, with nothing that varies between
runs. See [run](/reference/json/#run).

### Run report

An optional, separate JSON file (`--run-report`) that records how a run went: times, threads,
memory, platform and exit status, without user or host names. See
[keep a run report](/how-to/reproducibility/#keep-a-run-report).

### Schema version

The version of a JSON output's format (`schema_version`). Its minor number changes when keys are
added (results schema 1.1.0 added similarity-weighted entropy), its major number when the
meaning of existing fields changes. See [JSON outputs](/reference/json/).
