---
title: Your first analysis
description: A guided first analysis of the MERS-CoV spike protein alignment from the DiMA paper — run dima, read the summary and one position, write tables, find conserved sequences and cite the run.
---

This tutorial analyses the alignment behind Fig. 1C of the DiMA paper [@tharanga2025]: 191
MERS-CoV spike protein sequences. It takes a few minutes and shows every kind of result DiMA
produces. You need `dima` ([Install DiMA](/getting-started/install/)) and, for a few steps,
[`jq`](https://jqlang.org/).

## 1. Get the alignment

```sh
curl -fLO https://raw.githubusercontent.com/ShanWeera/dima-perf/master/samples/mers_spike_aa.fasta
grep -c '>' mers_spike_aa.fasta        # 191 sequences
head -c 200 mers_spike_aa.fasta
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

DiMA detects that the sequences are proteins, uses 9-mers and the paper's support threshold of
100, and writes the results to `mers.json`. On the terminal it prints a one-line summary of the
run (and warnings, if there were any).

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
  "lowest_entropy": { "position": 7, "entropy": 0.0, "tied_position_count": 455 },
  "highest_entropy": { "position": 26, "entropy": 1.5878251756456703, "tied_position_count": 1 }
}
```

The alignment has 1,353 columns, so there are $1{,}353 - 9 + 1 = 1{,}345$ positions of 9-mers.
Every position is supported by more than 100 sequences, so all are **eligible**: their entropy is
corrected for sample size and they count in the summary. `average_entropy` is the
[eligible mean](/reference/glossary/#eligible-mean): the average entropy of the eligible
positions, each counted once. The spike protein is mostly conserved — mean entropy 0.13 bits,
455 positions with a single variant — with the highest diversity at position 26.

## 4. Look at one position

Position 25 is the one shown in the paper's Fig. 1C:

```sh
jq '.results[24] | {position, support, entropy, observed_entropy, total_variants_incidence,
    distinct_variants_incidence, sample_coverage}' mers.json
jq -r '.results[24].diversity_motifs[] | [.sequence, .count, .motif_long] | @tsv' mers.json
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

- 170 of the 191 sequences have a complete 9-mer at columns 25–33 (the others have a gap or an
  ambiguous residue there): the **support** is 170.
- The most common variant, the **Index**, is carried by 129 sequences (75.88%); the second most
  common is the **Major**; variants seen more than once are **Minor**; variants seen once are
  **Unique**.
- 41 sequences (24.12%) carry a non-Index variant (`total_variants_incidence`); they carry 9
  distinct variants, 21.95% of 41 (`distinct_variants_incidence`; 22.0% in the paper).
- The observed entropy is 1.4224 bits; corrected for sample size it is 1.4617 bits (`entropy`).
  The paper shows 1.38 bits for this position, from one run of its random correction with a
  threshold of 30; [the explanation](/explanation/entropy/#the-exact-expectation) shows why both
  agree.
- The sample coverage, 0.98, estimates that one more sequence would almost certainly carry a
  variant already seen.

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
similarity-weighted entropy is less than a third of the observed entropy. The one-line summary
of this run also ends with `similarity-weighted mean 0.0363 bits (observed 0.1207)`. Read
[Add similarity-weighted entropy](/how-to/analyse/#add-similarity-weighted-entropy-optional)
before you use it: it explains the other values it adds and when to be careful.

## 5. Write tables

For R, Python or a spreadsheet, write the same results as four tidy tables:

```sh
dima analyze -i mers_spike_aa.fasta -o mers.json --tsv-dir tables
ls tables
```

```text
mers_spike_aa.hcs.tsv  mers_spike_aa.metadata.tsv  mers_spike_aa.positions.tsv  mers_spike_aa.variants.tsv
```

`positions` has one row per position, `variants` one row per variant, `hcs` one row per conserved
region; `metadata` stays empty until you describe the headers (step 7). See
[Choose and read outputs](/how-to/outputs/) for R and pandas recipes.

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

At the default threshold of 100%, DiMA finds 44 historically conserved sequences: stretches
where every supporting sequence carries the same residues. Try `--hcs-min-incidence 95` to allow
rare variants. See [Find conserved sequences](/how-to/conserved-sequences/).

## 7. Count variants per host

```sh
dima analyze -i mers_spike_aa.fasta -o mers.meta.json \
    --header-format 'Accession|Host|Geolocation|Year' --metadata-fields 'Host|Geolocation|Year'
jq '.results[24].diversity_motifs[1].metadata.Host' mers.meta.json
```

```json
{ "Camelus": 2, "Camelus dromedarius": 9, "Homo sapiens": 3 }
```

The Major variant at position 25 was found in camels and in humans. DiMA also warns that two
records have the year `nan` — a value that usually means "missing" — so that you can decide what
it means. See [Stratify by header metadata](/how-to/metadata/).

## 8. Cite the analysis

```sh
dima cite --from mers.json
```

prints a methods paragraph describing this run and the references of the methods it used,
including the DiMA paper and the exact version of the software. See [Citing DiMA](/reference/citing/).

## Next steps

- [Analyse an alignment](/how-to/analyse/): choose k, the support threshold, the alphabet and the
  estimator for your data.
- [How DiMA measures diversity](/explanation/method/): what every number means.
- [Use the desktop app](/how-to/desktop-app/) to explore the same results interactively.
- [Frequently asked questions](/getting-started/faq/) and the [Glossary](/reference/glossary/) of
  the terms in the results.

## References

[^ref]
