---
title: TSV tables
description: The four tidy tab-separated tables DiMA writes (positions, variants, metadata, conserved sequences), their columns, missing values and quoting.
---

DiMA writes four **tidy** tables: one observation per row, one variable per column, one table
per kind of observation [@wickham2014; @broman2018]. They hold the same values as the
[JSON results](/reference/json/), arranged for R, pandas, spreadsheets and command-line tools.

| Table | One row per | Written by |
|---|---|---|
| `positions` | position | `--tsv-dir DIR`, or `-o FILE.tsv --table positions` |
| `variants` | variant at a position | `--tsv-dir DIR`, or `-o FILE.tsv --table variants` |
| `metadata` | variant × header field × value | `--tsv-dir DIR`, or `-o FILE.tsv --table metadata` |
| `hcs` | conserved region | `--tsv-dir DIR`, `-o FILE.tsv --table hcs`, `--hcs-output FILE.tsv`, `dima hcs -O tsv` |

With `--tsv-dir DIR` (created if needed), the four files are named
`<name>.positions.tsv`, `<name>.variants.tsv`, `<name>.metadata.tsv` and `<name>.hcs.tsv`,
where `<name>` is the query name with characters other than letters, digits, `.`, `-` and `_`
replaced by `_`. `dima view` converts existing results the same way.

## Format

- Tab-separated, UTF-8, one header row, lines ending in a line feed.
- Quoting follows the CSV specification with a tab delimiter [@rfc4180]: a field is quoted with `"` only when
  it contains a tab, a quote or a line break, and quotes inside it are doubled. Header field
  names and values (in the `metadata` table) are the only fields that can need it.
- **`NA`** marks an undefined or not requested value (`null` in JSON). Only the columns marked
  "or NA" below can hold it.
- Numbers are written in their shortest exact form (for example `75.88235294117646`); booleans as
  `true` and `false`.
- Files written with `-o` can be compressed by name (`.tsv.gz`, `.tsv.zst`); `--tsv-dir` writes
  plain files.

:::caution[Reading NA safely]
Text columns can legitimately contain strings that R and pandas treat as missing by default: the
protein 2-mer `NA` (Asn-Ala), the 3-mer `NAN`, or a header value such as `nan` or `None`. Tell
your reader that only `NA` means missing, and only in the `positions` table; the other tables
have no missing values. Recipes are in [Choose and read outputs](/how-to/outputs/#read-the-tables-in-r-and-python).
:::

## positions

| Column | Type | Meaning |
|---|---|---|
| `position` | integer | 1-based position (first column of the k-mer) |
| `low_support` | `NS`, `LS`, `ELS` or NA | Support class; NA when eligible (support > $T$) |
| `support` | integer | Sequences with a valid k-mer |
| `distinct_variants_count` | integer | Distinct non-Index variants |
| `entropy` | number or NA | Entropy in bits by `entropy_method`; NA without support |
| `observed_entropy` | number or NA | Observed (plug-in) entropy; NA without support |
| `entropy_method` | text | `dima`, `chao_wang_jost`, `miller_madow`, `plug_in`, `not_estimable` |
| `entropy_fallback` | text or NA | Why the requested estimator was not used |
| `distinct_variants_incidence` | number or NA | Non-Index variants per 100 non-Index sequences |
| `total_variants_incidence` | number or NA | Non-Index sequences per 100 supporting sequences |
| `sample_coverage` | number or NA | Estimated sample coverage (NA when support < 2) |
| `below_min_coverage` | `true`, `false` or NA | Coverage below `--min-coverage` (NA without the option) |
| `similarity_weighted_entropy` | number or NA | [Similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) in bits (NA without `--similarity-weighted-entropy` or without support) |
| `normalised_observed_entropy` | number or NA | Observed entropy divided by $\log_2 D$, where $D$ is the number of distinct k-mers, Index included (NA without the option or with fewer than two distinct k-mers) |
| `normalised_similarity_weighted_entropy` | number or NA | Similarity-weighted entropy divided by the same $\log_2 D$ (NA without the option or with fewer than two distinct k-mers) |

## variants

| Column | Type | Meaning |
|---|---|---|
| `position` | integer | Position |
| `sequence` | text | The k-mer |
| `count` | integer | Sequences carrying it |
| `incidence` | number | count / support × 100 |
| `motif_short` | `I`, `Ma`, `Mi`, `U` | Motif class |
| `motif_long` | `Index`, `Major`, `Minor`, `Unique` | Motif class, spelled out |

Rows are ordered by position, then as in the JSON (count descending, then sequence).

## metadata

Empty (header row only) without `--header-format`.

| Column | Type | Meaning |
|---|---|---|
| `position` | integer | Position |
| `sequence` | text | The k-mer |
| `field` | text | Stratified header field, in header-format order |
| `value` | text | Cleaned header value (never empty: empty values get the fill value) |
| `count` | integer | Sequences with this k-mer at this position and this value |

The counts of one variant and one field add up to the variant's count. This table grows with
positions × variants × values: stratifying by an identifier-like field (an accession) adds one
row per sequence for every variant; see
[Stratify by header metadata](/how-to/metadata/#identifier-like-fields).

## hcs

| Column | Type | Meaning |
|---|---|---|
| `region_id` | integer | Region number |
| `start_column`, `end_column`, `length` | integers | Alignment columns covered |
| `min_incidence` | number | Lowest Index incidence among the members (%) |
| `min_support` | integer | Lowest support among the members |
| `conservation_lower_bound_95` | number | Lowest exact 95% Clopper–Pearson lower bound (proportion) |
| `flagged` | `true`, `false` | The region includes positions with support ≤ $T$ |
| `low_support_position_count` | integer | How many |
| `sequence` | text | The conserved sequence |

The member positions and the list of low-support positions are in the
[JSON output](/reference/json/#conserved-sequences). In `dima analyze` the table follows
`--hcs-min-incidence` and `--hcs-eligible-only`; `dima view` always uses the defaults (100%, all
positions), and `dima hcs` any other setting.

## References

[^ref]
