---
title: Your first analysis
description: A guided first analysis of the MERS-CoV spike protein alignment from the DiMA paper — run dima, read the summary and one position, write tables, find conserved sequences and cite the run.
---

This tutorial analyses the alignment behind Fig. 1C of the DiMA paper [@tharanga2025]: 191
MERS-CoV spike protein sequences. It takes a few minutes and shows every kind of result DiMA
produces. You need `dima` ([Install DiMA](/getting-started/install/)) and, for a few steps,
[`jq`](https://jqlang.org/).

The commands are for a POSIX shell: a terminal on Linux or macOS, or, on Windows,
[WSL](https://learn.microsoft.com/windows/wsl/) or Git Bash, where they work as written. The
`dima` commands themselves work the same in PowerShell and the Command Prompt; `curl`, `grep`,
`head`, `cut` and the line continuation `\` are the shell's. Run them in an empty folder: every
file is written there.

## 1. Get the alignment

```sh
curl -fLO https://raw.githubusercontent.com/ShanWeera/dima-perf/master/samples/mers_spike_aa.fasta
grep -c '>' mers_spike_aa.fasta        # 191 sequences
head -n 3 mers_spike_aa.fasta
```

```text
>YP_009047204.1 |Homo sapiens||2012
MIHSVFLLMFLLTPTESYVDVGPDSVKSACIEVDIQQTFFDKTWPRPIDVSKADGIIYPQ
GRTYSNITITYQGLFPYQGDHGDMYVYSAGHATGTTPQKLFVANYSQDVKQFANGFVVRI
```

It is an ordinary FASTA alignment, wrapped at 60 characters, with Windows (CRLF) line endings —
DiMA reads it like any other layout. The headers carry the accession, the host, the country and
the year, separated by `|`.

## 2. Run the analysis

```sh
dima analyze -i mers_spike_aa.fasta -o mers.json
```

```text
analysed 191 sequences, 1345 positions (1345 eligible) in 0.01 s; entropy mean 0.1261, min 0.0000, max 1.5878 bits
Cite: Tharanga et al. 2025, doi:10.1093/bib/bbae607 (`dima cite --from mers.json` lists this run's references)
```

DiMA detects that the sequences are proteins, cuts each sequence into overlapping
[9-mers](/reference/glossary/#k-mer-and-k) (windows of 9 residues, one starting at every
column), uses the paper's [support threshold](/reference/glossary/#support-threshold) of 100,
and writes the results to `mers.json`. On the terminal it prints the
[summary line](/reference/glossary/#summary-line) of the run — after any warnings — and, in a
terminal only, a `Cite:` line with the command that lists the references of this run (step 8).
Step 3 explains the numbers; the time differs from machine to machine.

## 3. Read the summary

```sh
jq '{sequence_count, position_count, eligible_position_count,
     average_entropy, lowest_entropy, highest_entropy}' mers.json
```

```json
{
  "sequence_count": 191,
  "position_count": 1345,
  "eligible_position_count": 1345,
  "average_entropy": 0.12613379461699337,
  "lowest_entropy": {
    "position": 7,
    "entropy": 0.0,
    "tied_position_count": 455
  },
  "highest_entropy": {
    "position": 26,
    "entropy": 1.5878251756456703,
    "tied_position_count": 1
  }
}
```

The alignment has 1,353 columns, so there are $1{,}353 - 9 + 1 = 1{,}345$ positions of 9-mers. A
position's [support](/reference/glossary/#support) is the number of sequences with a
[valid 9-mer](/reference/glossary/#valid-k-mer) there: one without a gap or an ambiguous residue.
Every position is supported by more than 100 sequences, so all are
[eligible](/reference/glossary/#eligible-position): they count in the summary.
`average_entropy` is the [eligible mean](/reference/glossary/#eligible-mean): the average entropy
of the eligible positions, each counted once. The spike protein is mostly conserved — mean
entropy 0.1261 bits, 455 positions with a single variant — with the highest diversity at
position 26.

Entropy is measured in [bits](/reference/glossary/#bit-and-nat): 0 when every sequence carries
the same 9-mer, more as the sequences spread over more variants.

## 4. Look at one position

Position 25 is the one shown in the paper's Fig. 1C:

```sh
jq '.results[24] | {position, support, entropy, observed_entropy, total_variants_incidence,
    distinct_variants_incidence, sample_coverage}' mers.json
jq -r '.results[24].diversity_motifs[] | [.sequence, .count, .motif_long] | @tsv' mers.json
```

```json
{
  "position": 25,
  "support": 170,
  "entropy": 1.4617012375322422,
  "observed_entropy": 1.4223886721629135,
  "total_variants_incidence": 24.11764705882353,
  "distinct_variants_incidence": 21.951219512195124,
  "sample_coverage": 0.9765630417196348
}
```

```text
SVKSACIEV	129	Index
SFKSACIEV	14	Major
SVKPACIEV	9	Minor
SAKSACIEV	6	Minor
SLKSACIEV	4	Minor
SVKSACIAV	4	Minor
SAKSVCIEV	1	Unique
SIKSACIEV	1	Unique
SVKSACIKV	1	Unique
SVKSVCIEV	1	Unique
```

- 170 of the 191 sequences have a valid 9-mer at columns 25–33 (the others have a gap or an
  [ambiguity code](/reference/glossary/#residue-gap-ambiguity-code-stop-frameshift-and-unknown-character),
  such as `X`, there): the **support** is 170.
- Each distinct 9-mer is a [variant](/reference/glossary/#variant), and DiMA names them by
  [motif](/reference/glossary/#index-major-minor-and-unique): the most common one, the
  **Index**, is carried by 129 sequences (75.88%); the **Major** is the most common of the other
  variants seen more than once (`SFKSACIEV`, 14 sequences); the other variants seen more than
  once are **Minor**; variants seen once are **Unique**.
- 41 sequences (24.12%) carry a non-Index variant
  ([`total_variants_incidence`](/reference/glossary/#total-variants-incidence)); they carry 9
  distinct variants, 21.95% of 41
  ([`distinct_variants_incidence`](/reference/glossary/#distinct-variants-incidence); 22.0% in
  the paper).
- The [observed entropy](/reference/glossary/#observed-entropy), computed from these counts as
  they are, is 1.4224 bits; [corrected](/reference/glossary/#corrected-entropy) for sample size
  it is 1.4617 bits (`entropy`). The paper shows 1.38 bits for this position, from one run of its
  random correction with a threshold of 30;
  [the explanation](/explanation/entropy/#the-exact-expectation) shows why both agree.
- The [sample coverage](/reference/glossary/#sample-coverage), 0.98, estimates that one more
  sequence would almost certainly carry a variant already seen.

With the paper's estimator, the default, an eligible position's entropy is corrected *where
possible*: the correction needs at least five points (the full data set included; a minimum
taken from DiMA v5.0.9, which the paper does not state), so a position is corrected only when its
support is at least 1.25 T, 125 sequences for T = 100. Positions with support between 100 and
125 keep their observed entropy; they are still eligible and in the summary. In this alignment
every position is supported by 168 to 181 sequences, so every entropy is corrected:

```sh
jq '[.results[].support] | {min: min, max: max}' mers.json
```

```json
{
  "min": 168,
  "max": 181
}
```

For smaller alignments, see
[Choose the support threshold](/how-to/analyse/#choose-the-support-threshold).

DiMA can also compute **similarity-weighted entropy**, in which variants that differ at few
residues count for less [@wu2025]. It is not part of the paper's method, so you ask for it:

```sh
dima analyze -i mers_spike_aa.fasta --similarity-weighted-entropy -o mers.swe.json
jq '.results[24] | {observed_entropy, similarity_weighted_entropy}' mers.swe.json
```

```json
{
  "observed_entropy": 1.4223886721629135,
  "similarity_weighted_entropy": 0.42754448388901667
}
```

At position 25 almost every variant differs from the Index at a single residue, so the
similarity-weighted entropy is less than a third of the observed entropy. The summary line of
this run also ends with `similarity-weighted mean 0.0363 bits (observed 0.1207)`. Similarity-weighted
entropy is not corrected for sample size, so the line sets its mean beside the mean *observed*
entropy of the same positions (0.1207), not beside the corrected mean of the line (0.1261): the
two values in brackets compare like with like. Read
[Add similarity-weighted entropy](/how-to/analyse/#add-similarity-weighted-entropy-optional)
before you use it: it explains the other values it adds and when to be careful.

## 5. Write tables

For R, Python or a spreadsheet, write the same results as four
[tidy tables](/reference/glossary/#tidy-table) as well:

```sh
dima analyze -i mers_spike_aa.fasta -o mers.json --tsv-dir tables
ls tables
```

```text
mers_spike_aa.hcs.tsv
mers_spike_aa.metadata.tsv
mers_spike_aa.positions.tsv
mers_spike_aa.variants.tsv
```

The tables are named after the input file, without its `.fasta`. `positions` has one row per
position, `variants` one row per variant, `hcs` one row per conserved region; `metadata` stays
empty until you describe the headers (step 7). The tables hold no settings: keep `mers.json`,
which records them. See [Choose and read outputs](/how-to/outputs/) for R and pandas recipes.

## 6. Find conserved sequences

```sh
dima hcs -i mers.json -O tsv | cut -f 1-4,10 | head -4
```

```text
region_id	start_column	end_column	length	sequence
1	7	22	16	LLMFLLTPTESYVDVG
2	35	83	49	IQQTFFDKTWPRPIDVSKADGIIYPQGRTYSNITITYQGLFPYQGDHGD
3	107	125	19	QDVKQFANGFVVRIGAAAN
```

`dima hcs` also prints its own summary line on standard error,
`44 conserved regions with an Index incidence of at least 100%`. At the default threshold of
100%, DiMA finds 44 [historically conserved sequences](/reference/glossary/#historically-conserved-sequence-hcs):
stretches where every supporting sequence carries the same residues. `--hcs-min-incidence 95`
also allows rare variants; read
[What a region's numbers mean](/explanation/conserved-sequences/#what-a-regions-numbers-mean)
before you use a lower threshold, because a region's numbers then describe each of its positions,
not the whole stretch. See [Find conserved sequences](/how-to/conserved-sequences/).

## 7. Count variants per host

```sh
dima analyze -i mers_spike_aa.fasta -o mers.meta.json \
    --header-format 'Accession|Host|Geolocation|Year' --metadata-fields 'Host|Geolocation|Year'
jq '.results[24].diversity_motifs[1].metadata.Host' mers.meta.json
```

```text
warning[missing_like_values]: field "Year": 2 records have values that usually mean "missing"; they are kept as literal values
  e.g. "nan" (2 records)
  hint: if they mean missing, empty them in the headers so they receive the fill value, or edit them consistently
analysed 191 sequences, 1345 positions (1345 eligible) in 0.02 s; entropy mean 0.1261, min 0.0000, max 1.5878 bits; 1 warning (see above)
```

```json
{
  "Camelus": 2,
  "Camelus dromedarius": 9,
  "Homo sapiens": 3
}
```

The Major variant at position 25 was found in camels and in humans. The warning tells that two
records have the year `nan` — a value that usually means "missing" — so that you can decide what
it means; the summary line counts it. See [Stratify by header metadata](/how-to/metadata/).

## 8. Cite the analysis

```sh
dima cite --from mers.json
```

prints a methods paragraph describing this run and the references of the methods it used,
including the DiMA paper and the exact version of the software. It is the command the `Cite:`
line of step 2 suggests. See [Citing DiMA](/reference/citing/).

## Next steps

- [Analyse an alignment](/how-to/analyse/): choose k, the support threshold, the alphabet and the
  estimator for your data.
- [How DiMA measures diversity](/explanation/method/): what every number means.
- [Use the desktop app](/how-to/desktop-app/) to explore the same results interactively.
- [Frequently asked questions](/getting-started/faq/) and the [Glossary](/reference/glossary/) of
  the terms in the results.

## References

[^ref]
