---
title: JSON outputs
description: Field-by-field reference of DiMA's JSON outputs — the results, the conserved sequences and the run report — with links to their JSON Schemas.
---

DiMA writes three kinds of JSON document. Each has a JSON Schema (Draft 2020-12) whose `$id`
is its published address:

| Document | Written by | Schema |
|---|---|---|
| Results | `dima analyze` (`-o file.json`, or standard output), `dima view -O json` | [results.schema.json](https://shanweera.github.io/dima-perf/schemas/1.0/results.schema.json) |
| Conserved sequences | `dima analyze --hcs-output file.json`, `dima hcs` | [hcs.schema.json](https://shanweera.github.io/dima-perf/schemas/1.0/hcs.schema.json) |
| Run report | `dima analyze --run-report file.json`, the desktop app | [run-report.schema.json](https://shanweera.github.io/dima-perf/schemas/1.0/run-report.schema.json) |

Conventions shared by all three:

- **Versioned.** `schema_version` is `"1.1.0"` for the results, `"1.0.0"` for the conserved
  sequences and `"1.1.0"` for the run report (SemVer); each changes only when its own schema
  does. Results schema 1.1.0 added
  [similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in): three
  values per position, a summary and a parameter. A new minor version may add keys, diagnostic codes or
  values of an enumerated field (an estimator, method or fallback name); a new major version
  may change the meaning of existing ones. `dima view`, `dima hcs`, `dima cite` and the desktop
  app refuse results of another major version. Results of a newer minor version are read,
  leaving out keys this version does not know (`dima view` warns when it writes them back); a
  value it does not know stops the reading with a message naming the newer schema, because
  guessing its meaning could mislead. Results of an older minor version are read as runs
  without the options their schema did not have: results written by dima 1.0.0 (schema 1.0.0)
  are read as runs without similarity-weighted entropy, so writing them again gives `null` for
  its values and `false` for its option (`dima view` says so in a note). Such files validate
  against the 1.0.0 schema, `schemas/results.schema.json` of release 1.0.0 (tag `v1.0.0`), not
  against the schema now published at the address in its `$id`, which is 1.1.0 and requires the
  keys 1.1.0 added (see [Comparing versions](/how-to/reproducibility/#comparing-versions)).
- **Undefined or not requested is `null`.** A value that cannot be computed (an entropy without
  support, a $0/0$ ratio, a summary without eligible positions) or that was not requested (the
  similarity-weighted values of a run without `--similarity-weighted-entropy`) is `null`, never
  0 or NaN. Every number is finite.
- **Positions are 1-based** alignment columns.
- **Entropies are in bits**; incidences are percentages (0–100); coverages and lower bounds are
  proportions (0–1).
- Numbers are written in their shortest exact form; the JSON is compact unless `--pretty` is
  given or it is written to a terminal, and always ends with a newline. Files named `.json.gz`
  or `.json.zst` are compressed; DiMA reads them back by content.
- Every key is always present (no optional keys), so a reader can rely on the shape.

## Results

The results of `dima analyze` (JSONL output holds the same `results[]` entries, one per line,
without the summaries and `run`).

### Top level

| Key | Type | Meaning |
|---|---|---|
| `schema_version` | string | `"1.1.0"` |
| `query_name` | string | Name of the analysed protein or gene (see [the query name](/explanation/input/#the-query-name)) |
| `sequence_count` | integer | Sequences in the alignment |
| `support_threshold` | integer ≥ 1 | The support threshold $T$ |
| `kmer_length` | integer ≥ 1 | $k$ |
| `low_support_count` | integer | Positions with support ≤ $T$ (NS + LS + ELS) |
| `average_entropy` | number or null | Mean entropy over eligible positions (support > $T$) |
| `lowest_entropy` | [extremum](#extremum) or null | Lowest entropy among eligible positions |
| `highest_entropy` | [extremum](#extremum) or null | Highest entropy among eligible positions |
| `position_count` | integer | Positions, $L - k + 1$ |
| `eligible_position_count` | integer | Positions with support > $T$ |
| `support_label_counts` | object | `{"NS": …, "LS": …, "ELS": …, "eligible": …}` |
| `all_positions_summary` | object | [All-positions summary](#all_positions_summary) (DiMA v5.0.9's averaging) |
| `coverage_summary` | object | [Coverage summary](#coverage_summary) |
| `similarity_weighted_entropy_summary` | object or null | [Similarity-weighted entropy summary](#similarity_weighted_entropy_summary); null without `--similarity-weighted-entropy` |
| `run` | object | [Provenance of the run](#run) |
| `results` | array | One [position](#position) per k-mer position, in order |

#### extremum

| Key | Type | Meaning |
|---|---|---|
| `position` | integer ≥ 1 | First position with this value |
| `entropy` | number ≥ 0 | The value, in bits |
| `tied_position_count` | integer ≥ 1 | Positions with exactly this value |

#### all_positions_summary

| Key | Type | Meaning |
|---|---|---|
| `rule` | string | The averaging rule, in words |
| `position_count` | integer | Positions averaged (all of them) |
| `average_entropy` | number or null | Mean with NS positions as 0 bits and LS/ELS positions with their reported entropy (the observed entropy with the `dima` and `raw` estimators) |
| `lowest_entropy`, `highest_entropy` | [extremum](#extremum) or null | Extrema under the same rule (first position, 1-based) |

#### coverage_summary

| Key | Type | Meaning |
|---|---|---|
| `evaluated` | integer | Positions with a sample coverage (support ≥ 2) |
| `min`, `mean` | number or null | Lowest and mean sample coverage |
| `min_coverage` | number or null | `--min-coverage`, if given |
| `below_min_count` | integer or null | Positions below `--min-coverage`, if given |

#### similarity_weighted_entropy_summary

Present (not `null`) only with `--similarity-weighted-entropy`. It covers the eligible positions
(support > $T$), like the top-level entropy summary, and gives their mean observed entropy too,
so the two can be compared like for like. The normalised values are not summarised.

| Key | Type | Meaning |
|---|---|---|
| `position_count` | integer | Eligible positions |
| `average_entropy` | number or null | Mean similarity-weighted entropy, in bits; null when no position is eligible |
| `lowest_entropy`, `highest_entropy` | [extremum](#extremum) or null | Lowest and highest similarity-weighted entropy (first position, tie count) |
| `average_observed_entropy` | number or null | Mean observed (plug-in) entropy of the same positions, in bits |

### position

One entry of `results[]` (and one line of JSONL output).

| Key | Type | Meaning |
|---|---|---|
| `position` | integer ≥ 1 | Alignment column of the k-mer's first residue |
| `low_support` | `"NS"`, `"LS"`, `"ELS"` or null | Support 0, below $T$, equal to $T$; null when eligible |
| `support` | integer | Sequences with a valid k-mer here ($N$) |
| `distinct_variants_count` | integer | Distinct non-Index variants |
| `entropy` | number or null | Entropy in bits by `entropy_method`; null without support |
| `observed_entropy` | number or null | Plug-in (observed) entropy; null without support |
| `entropy_method` | string | `dima`, `chao_wang_jost`, `miller_madow`, `plug_in` or `not_estimable` |
| `entropy_fallback` | string or null | Why the requested estimator was not used: `support_at_or_below_threshold`, `too_few_points`, `degenerate_regression` (estimator `dima`), `single_observation` (Miller–Madow, Chao–Wang–Jost) |
| `distinct_variants_incidence` | number or null | Non-Index variants per 100 non-Index sequences; null when there are none ($0/0$) |
| `total_variants_incidence` | number or null | Non-Index sequences per 100 supporting sequences; null without support |
| `sample_coverage` | number or null | Estimated sample coverage; null when support < 2 |
| `below_min_coverage` | boolean or null | Coverage below `--min-coverage`; null without the option or without coverage |
| `similarity_weighted_entropy` | number or null | [Similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) $H_\text{sim}$ in bits; null without `--similarity-weighted-entropy` or without support |
| `normalised_observed_entropy` | number or null | Observed entropy divided by $\log_2$ of the number of distinct k-mers $D$, the Index included ($H_n$, between 0 and 1); null without the option or when $D < 2$ |
| `normalised_similarity_weighted_entropy` | number or null | Similarity-weighted entropy divided by $\log_2 D$ ($H_\text{nsim}$, between 0 and 1); null without the option or when $D < 2$ |
| `diversity_motifs` | array | The [variants](#variant), most frequent first; empty without support |

The formulas are in [Entropy and its correction](/explanation/entropy/) and
[Diversity motifs](/explanation/motifs/).

**Presence rule.** In every 1.x results file the three similarity-weighted values are either
all `null` at every position (the run did not request them) or given exactly where they are
defined: `similarity_weighted_entropy` at every position with support, the two normalised
values at every position with at least two distinct k-mers, and the summary is an object. The
schema and every DiMA reader check this; a file that breaks it was edited or damaged.

### variant

| Key | Type | Meaning |
|---|---|---|
| `sequence` | string | The k-mer, upper case |
| `count` | integer ≥ 1 | Sequences carrying it |
| `incidence` | number | `count` / support × 100 |
| `motif_short` | `"I"`, `"Ma"`, `"Mi"`, `"U"` | Motif class |
| `motif_long` | `"Index"`, `"Major"`, `"Minor"`, `"Unique"` | The same, spelled out |
| `metadata` | object or null | With `--header-format`: for each stratified field (in header-format order), an object mapping each value (sorted) to the number of this variant's sequences with that value; null without `--header-format` |

Example (MERS-CoV spike position 25, `--header-format 'Accession|Host|Geolocation|Year'
--metadata-fields 'Host|Geolocation|Year'`):

```json
{
  "sequence": "SFKSACIEV",
  "count": 14,
  "incidence": 8.235294117647058,
  "motif_short": "Ma",
  "motif_long": "Major",
  "metadata": {
    "Host": { "Camelus": 2, "Camelus dromedarius": 9, "Homo sapiens": 3 },
    "Geolocation": { "Oman": 2, "United Arab Emirates": 12 },
    "Year": { "2014": 3, "2015": 11 }
  }
}
```

### run

The deterministic provenance of the run: everything needed to interpret and reproduce the
results, and nothing that varies between runs (no times, host names, paths or thread counts —
those are in the [run report](#run-report)).

| Key | Meaning |
|---|---|
| `tool` | `{"name": "dima", "version": …}`: the DiMA version that wrote the file |
| `parameters` | Every analysis parameter as `{"value": …, "origin": …}`, where origin is `default`, `user` (given on the command line or in the app) or `derived` (from the data, such as the alphabet and the default k): `alphabet`, `kmer_length`, `support_threshold`, `estimator`, `similarity_weighted_entropy` (`true` with `--similarity-weighted-entropy`; `false`, origin `default`, otherwise and in files of schema 1.0.0), `validation`, `min_coverage`, `query_name`, `header_format`, `metadata_fields`, `header_fill` |
| `alphabet` | The alphabet decision: `resolved` (`protein`, `dna`, `rna`), `method` (`detected` or `user`), `records_examined`, `residues_examined`, `t_u_folded` (T or U letters folded to the alphabet's spelling) and `contradiction` (why the data disagree with an explicit `--alphabet`, or null) |
| `alignment_length` | Alignment columns $L$ |
| `input` | The input: `name` (file name without directories, `-` for standard input), `size_bytes` (stored size), `checksum` (`{"type": "sha-256", "value": …}` of the stored, possibly compressed, bytes) and `compression` (`none`, `gzip`, `bzip2`, `zstd`) |
| `validation` | What reading found: `mode`, `records`, `lowercase_folded`, `whitespace` (`trailing` and `interior` line counts), `characters` (every byte found in sequence lines: `char` — null unless the byte is a visible ASCII character, so also for the space —, `byte`, `class` and `count`), `unknown_total`, `header_utf8_replacements` and `header_cleaning` (null without `--header-format`; otherwise `unicode_version`, `trimmed`, `invisible_removed` by code point, `nfc_changed`, `fill_uses`; they count the values of stratified fields) |
| `metadata` | Null without `--header-format`; otherwise `fields` (all field names), `stratified` (the stratified ones), `probe_records`, `mismatches` (`count` and the `first` 20 headers with the wrong number of fields: `record`, `line`, `header`, `expected`, `found`) and `fill_uses` (stratified values replaced by the fill value: empty values, and every stratified field of a header with the wrong number of fields) |
| `entropy` | `estimator`, `log_base` (2) and `rarefaction`: for the `dima` estimator, the constants of the paper's procedure (`grid` `paper_fig2`: Fig. 2's grid with the step and start rounding of DiMA v5.0.9; `size_rounding` `ceil`, `start_rounding` `ceil_to_5`, `sampling` `expected_with_replacement`, `min_points` 5, DiMA v5.0.9's minimum); null for other estimators |
| `diagnostics` | The [diagnostics](/reference/diagnostics/) of the run: `code`, `severity` (`info` or `warning`), `message`, `count`, up to 5 `examples` and a `hint` (or null) |
| `references` | The references of the methods the run used, for `dima cite --from`: `key`, `citation` (full text), `doi` and `url` (either may be null). The first two are always the DiMA paper and this software version |

## Conserved sequences

Written by `--hcs-output` and `dima hcs`. See
[Historically conserved sequences](/explanation/conserved-sequences/).

| Key | Meaning |
|---|---|
| `schema_version` | `"1.0.0"` |
| `tool` | `{"name": "dima", "version": …}`: the version that computed the regions |
| `source` | The results the regions were computed from: `query_name`, `kmer_length`, `support_threshold`, `sequence_count`, `input_checksum` (as in `run.input`), `tool_version` and `results_schema_version` |
| `parameters` | `threshold_percent` (above 50, at most 100), `merge_rule` (`overlap_or_adjacent`), `eligibility` (`all_positions`, or `support_gt_threshold` with `--hcs-eligible-only`) and `confidence` (`{"method": "clopper_pearson", "level": 0.95}`) |
| `region_count`, `flagged_region_count` | Regions, and regions that include positions with support ≤ $T$ |
| `regions` | The [regions](#region), in column order |
| `diagnostics` | `hcs_low_support_regions` when regions are flagged (same shape as in the results) |
| `references` | The references of the method |

### region

| Key | Type | Meaning |
|---|---|---|
| `id` | integer ≥ 1 | Region number |
| `start_column`, `end_column`, `length` | integers | Alignment columns covered |
| `sequence` | string | The conserved sequence |
| `min_incidence` | number | Lowest Index incidence among the members (%) |
| `min_support` | integer ≥ 2 | Lowest support among the members |
| `conservation_lower_bound_95` | number | Lowest exact 95% Clopper–Pearson lower bound among the members (proportion) |
| `low_support_positions` | array of integers | Every position in the region's span with support ≤ $T$, member or not |
| `flagged` | boolean | `low_support_positions` is not empty |
| `split_from_previous` | boolean | The region starts where a k-mer overlapping the previous region disagreed with it |
| `members` | array | The qualifying positions: `position`, `kmer`, `count`, `support`, `incidence` and `conservation_lower_bound_95` |

## Run report

Written by `--run-report PATH` (and the desktop app's run report export), also when the run fails
or is interrupted. It describes how the run went and is kept separate from the results, which
stay byte-identical from run to run. It never contains host or user names, directories, the raw
command line or environment values.

| Key | Meaning |
|---|---|
| `schema_version` | `"1.1.0"` (1.1.0 added `threads.started`) |
| `tool` | `name` (`dima` or `dima-gui`), `version` and `target` (the Rust target triple of the binary, e.g. `x86_64-unknown-linux-gnu`) |
| `command` | The equivalent command, rebuilt from the parsed options: long option names, file names without directories |
| `status`, `exit_code`, `error` | `completed`, `failed` or `cancelled`; the [exit code](/reference/exit-codes/); the error report with directories removed (null unless failed) |
| `start_time`, `end_time` | UTC, RFC 3339 with milliseconds |
| `threads` | `count` requested, `started` (the same, or fewer when the system refused some; null if the run stopped before starting them) and `source` (`--threads`, `RAYON_NUM_THREADS`, `available_parallelism`, the desktop app's settings, or the name of the scheduler variable that limited the count); null if the run stopped earlier |
| `platform` | `os`, `arch`, `logical_cpus` |
| `resources` | `wall_seconds`, `cpu_user_seconds`, `cpu_system_seconds` and `peak_rss_bytes` (null where not measured, as in the desktop app), `waited_for_input_seconds` (time the counting threads waited for the reader; null when the analysis failed or was cancelled) |
| `phases` | Wall time of each phase reached: `opening`, `counting`, `finalising`, `writing` (for a run that failed or was cancelled, the phases it had reached, the last one until it stopped) |
| `work` | `records` read and `positions` analysed |
| `inputs`, `outputs` | Each file's `role` (`alignment`, `results`, `results_jsonl`, `positions_table`, `variants_table`, `metadata_table`, `hcs_table`, `hcs`), `name`, `size_bytes` and `sha256` of its stored bytes |
