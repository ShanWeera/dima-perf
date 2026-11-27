---
title: JSON outputs
description: Field-by-field reference of DiMA's JSON outputs — the results, the conserved sequences and the run report — with links to their JSON Schemas.
---

DiMA writes three kinds of JSON document. Each has a JSON Schema (Draft 2020-12) whose `$id`
is its published address. The `1.0` in these addresses stands for major version 1: the address
always serves the newest schema of that major version, so a file of an older minor version can
need the schema of its own release (see below).

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
  values per position, a summary and a parameter. A new minor version may add keys or values of
  an enumerated field (an estimator, method or fallback name); a new major version may change the
  meaning of existing ones. Diagnostic codes are free strings, so a new release of DiMA may add
  codes without changing the schema version: DiMA 1.2.0 added `uncorrected_eligible_positions`
  under results schema 1.1.0. `dima view`, `dima hcs`, `dima cite` and the desktop
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
- Numbers are written in their shortest exact form: `75.88235294117646`, and in exponent form
  below 0.00001 (`3.08e-7`), which every JSON reader accepts. The results and the conserved
  sequences are compact unless `--pretty` is given or they are written to a terminal; the run
  report is always indented. Every document ends with a newline. Files named `.json.gz` or
  `.json.zst` are compressed; DiMA reads them back by content. Results compressed with bzip2, xz
  or another format DiMA does not write are refused with the name of the format (see
  [Troubleshooting](/how-to/troubleshooting/)).
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
| `low_support_count` | integer | Positions with support ≤ $T$ (NS + LS + ELS; see [low support](/reference/glossary/#low-support)) |
| `average_entropy` | number or null | Mean entropy over eligible positions (support > $T$), each position counted once: the [eligible mean](/reference/glossary/#eligible-mean); null when no position is eligible |
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
| `lowest_entropy`, `highest_entropy` | [extremum](#extremum) or null | Extrema under the same rule (first position, 1-based). The lowest can be an NS position: on the HIV-1 *pol* protein alignment it is position 30, 0 bits, tied with 671 other positions, 551 of them NS |

#### coverage_summary

It covers every position with a sample coverage, eligible or not: LS and ELS positions are
included, so its minimum can come from a position with few sequences. On the HIV-1 *pol* protein
alignment the minimum, 0.1818, is at LS position 29, whose 4 sequences carry 4 different 9-mers;
over its eligible positions alone the minimum is 0.7338 (mean 0.9850 against 0.9543 for all 518).

| Key | Type | Meaning |
|---|---|---|
| `evaluated` | integer | Positions with a sample coverage (support ≥ 2), eligible or not |
| `min`, `mean` | number or null | Lowest and mean sample coverage of those positions; null when none is evaluated |
| `min_coverage` | number or null | `--min-coverage`; null when it was not given |
| `below_min_count` | integer or null | Evaluated positions below `--min-coverage`, LS and ELS included; null without the option |

#### similarity_weighted_entropy_summary

Present (not `null`) only with `--similarity-weighted-entropy`. It covers the eligible positions
(support > $T$), like the top-level entropy summary, and gives their mean observed entropy too,
so the two can be compared like for like. The normalised values are not summarised.

| Key | Type | Meaning |
|---|---|---|
| `position_count` | integer | Eligible positions |
| `average_entropy` | number or null | Mean similarity-weighted entropy, in bits, each eligible position counted once (the desktop app's *eligible mean* line when its chart shows similarity-weighted entropy); null when no position is eligible |
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
| `entropy` | number or null | The [reported entropy](/reference/glossary/#entropy-reported-entropy) in bits, by `entropy_method`; null without support |
| `observed_entropy` | number or null | Plug-in (observed) entropy; null without support |
| `entropy_method` | string | The method that produced `entropy` (below) |
| `entropy_fallback` | string or null | Why the requested estimator was not used (below); null when it was used, with `raw`, and without support |
| `distinct_variants_incidence` | number or null | Non-Index variants per 100 non-Index sequences; null when there are none ($0/0$) |
| `total_variants_incidence` | number or null | Non-Index sequences per 100 supporting sequences; null without support |
| `sample_coverage` | number or null | Estimated sample coverage; null when support < 2 |
| `below_min_coverage` | boolean or null | Coverage below `--min-coverage`; null without the option, and where `sample_coverage` is null (support below 2) even with it |
| `similarity_weighted_entropy` | number or null | [Similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) $H_\text{sim}$ in bits; null without `--similarity-weighted-entropy` or without support |
| `normalised_observed_entropy` | number or null | Observed entropy divided by $\log_2$ of the number of distinct k-mers $D$, the Index included ($H_n$, between 0 and 1); null without the option or when $D < 2$ |
| `normalised_similarity_weighted_entropy` | number or null | Similarity-weighted entropy divided by $\log_2 D$ ($H_\text{nsim}$, between 0 and 1); null without the option or when $D < 2$ |
| `diversity_motifs` | array | The [variants](#variant) (the name is DiMA v5.0.9's), most frequent first and, at equal counts, in byte order of the k-mer; empty without support |

The formulas are in [Entropy and its correction](/explanation/entropy/) and
[Diversity motifs](/explanation/motifs/).

**Method and fallback.** `entropy_method` names what produced `entropy`, and `entropy_fallback`
why it is not the estimator that was asked for:

| `entropy_method` | `entropy_fallback` | When |
|---|---|---|
| `dima` | null | the paper's correction, at an eligible position supported by at least 1.25 × T sequences, rounded up |
| `chao_wang_jost`, `miller_madow` | null | that estimator, at a position supported by at least 2 sequences, eligible or not |
| `plug_in` | null | estimator `raw`: the observed entropy everywhere |
| `plug_in` | `support_at_or_below_threshold` | estimator `dima`, support ≤ $T$ (LS, ELS) |
| `plug_in` | `too_few_points` | estimator `dima`, an eligible position with support below 1.25 × T, rounded up: fewer than the five regression points the correction needs (warning [`uncorrected_eligible_positions`](/reference/diagnostics/#uncorrected_eligible_positions)) |
| `plug_in` | `degenerate_regression` | estimator `dima`, $T = 2$ and support 3: every subsample has the same size, so no regression line exists |
| `plug_in` | `single_observation` | estimator `chao-wang-jost` or `miller-madow`, support 1 (0 bits) |
| `not_estimable` | null | no support (NS): `entropy` and `observed_entropy` are null |

With `plug_in`, `entropy` equals `observed_entropy`. On the HIV-1 *pol* protein alignment with the
defaults, 474 eligible positions are corrected and 2 (positions 42 and 43, support 105) are
`too_few_points`.

**Presence rule.** In every 1.x results file the three similarity-weighted values are either
all `null` at every position (the run did not request them) or given exactly where they are
defined: `similarity_weighted_entropy` at every position with support, the two normalised
values at every position with at least two distinct k-mers, and the summary is an object. The
schema and every DiMA reader check this. DiMA's readers also check that no
`similarity_weighted_entropy` is above the position's `observed_entropy` by more than rounding
(a relative $10^{-12}$): every weight is at most 1, so only rounding can make it exceed the
observed entropy, by a few units in the last digit. A file that breaks either rule was edited or
damaged.

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
| `parameters` | Every analysis parameter, with where its value came from ([below](#parameters)) |
| `alphabet` | The alphabet decision ([below](#alphabet)) |
| `alignment_length` | Alignment columns $L$ |
| `input` | The input: `name` (file name without directories; `-` for standard input and for descriptor paths such as `/dev/stdin` and process substitution, `<(…)`), `size_bytes` (stored size), `checksum` (`{"type": "sha-256", "value": …}` of the stored, possibly compressed, bytes) and `compression` (`none`, `gzip` — BGZF included —, `bzip2`, `zstd`) |
| `validation` | What reading found ([below](#validation)) |
| `metadata` | Header metadata; null without `--header-format` ([below](#metadata)) |
| `entropy` | The entropy settings ([below](#entropy)) |
| `diagnostics` | The [diagnostics](/reference/diagnostics/) of the run: `code`, `severity` (`info` or `warning`), `message`, `count` (what it counts depends on the code; see the [table of codes](/reference/diagnostics/#codes)), up to 5 `examples` and a `hint` (or null) |
| `references` | The references of the methods the run used, for `dima cite --from`: `key`, `citation` (full text), `doi` and `url` (either may be null). The first two are always the DiMA paper and this software version (key `dima_software`) |

#### parameters

Every analysis parameter as `{"value": …, "origin": …}`: `alphabet`, `kmer_length`,
`support_threshold`, `estimator`, `similarity_weighted_entropy` (`true` with
`--similarity-weighted-entropy`; `false`, origin `default`, otherwise and in files of schema
1.0.0), `validation`, `min_coverage`, `query_name`, `header_format`, `metadata_fields` and
`header_fill` (the value used: trimmed and NFC-normalised). The origin is:

- `user`: given on the command line or in the desktop app;
- `default`: the documented default (support threshold 100, estimator `dima`, …);
- `derived`: worked out from the data — the alphabet when it was detected (also with an explicit
  `--alphabet auto`), the default k of that alphabet, and the query name taken from the input file
  name.

Values are spelled as in the results, with underscores: `"estimator": {"value": "chao_wang_jost",
…}` for `--estimator chao-wang-jost`. Analysed from standard input (or `/dev/stdin`, or `<(…)`),
the query name is `Unknown Protein` (`Unknown DNA`, `Unknown RNA`) with origin `default`, and
`input.name` is `-`; give `--query-name` to name it.

#### alphabet

| Key | Meaning |
|---|---|
| `resolved` | `protein`, `dna` or `rna`: the alphabet used |
| `method` | `detected` (from the letters of the first records, with the rules of `esl_abc_GuessAlphabet` in Easel, the sequence library of HMMER) or `user` (`--alphabet`) |
| `records_examined` | The records of the first batch, whose letters detection examines: at most 10,000 records or 64 MiB, and at least the first 1,000 records (or all, if there are fewer). It is recorded also when the alphabet was given. The ToBRFV tm-2 reads of the repository examine 10,000 of their 10,833 records |
| `residues_examined` | The letters A–Z, in either case, in those records, ambiguity codes and letters outside the alphabet included: 236,346 on the MERS-CoV spike sample, its 235,756 residues and 590 ambiguity codes (X 587, Z 2, B 1) |
| `t_u_folded` | U letters read as T (DNA) or T letters read as U (RNA), in the whole input |
| `contradiction` | Why the residue composition contradicts an explicit `--alphabet`; null when it does not, and when the alphabet was detected |

#### validation

| Key | Meaning |
|---|---|
| `mode` | `strict`, `permissive` or `report` |
| `records` | Records read |
| `lowercase_folded` | Lower-case letters read as upper case |
| `whitespace` | `trailing`: sequence lines that end in white space; `interior`: lines with white space between residues (ignored) |
| `characters` | Every byte value found in sequence lines (line endings excluded), one entry per byte value in byte order, so `A` and `a` are separate entries (14 and 2 in the repository's `tests/golden/tiny/input.fasta`): `char` (null unless the byte is a visible ASCII character, so also for the space), `byte`, `class` (its [symbol class](/reference/glossary/#symbol-class)) and `count` |
| `unknown_total` | Characters of class `unknown`, whose k-mers were excluded (permissive and report modes; strict mode stops at the first) |
| `header_utf8_replacements` | Invalid UTF-8 sequences in headers (not bytes) replaced by U+FFFD. Headers are checked only with `--header-format`: without it the value is 0 |
| `header_cleaning` | Null without `--header-format`. Otherwise what cleaning changed in the values of [stratified fields](/reference/glossary/#stratified-field): `unicode_version` (the version of the Unicode tables used; another version can clean some values differently), `trimmed` (values whose surrounding white space was removed), `invisible_removed` (invisible characters removed, by code point), `nfc_changed` (values changed by [NFC](/reference/glossary/#nfc) normalisation) and `fill_uses` (as in `metadata`) |

#### metadata

Null without `--header-format`.

| Key | Meaning |
|---|---|
| `fields` | Every field name of `--header-format`, in order, trimmed and NFC-normalised |
| `stratified` | The stratified fields (`--metadata-fields`, or every field), in header-format order |
| `probe_records` | The records of the first batch (the same as `alphabet.records_examined`), whose headers the identifier-like check examines; `identifier_like_field` needs at least 100 |
| `mismatches` | `count` of the headers whose number of `|`-separated fields differs from the format, and the `first` 20 of them in input order: `record`, `line`, `header` (without the `>`), `expected` and `found` (numbers of fields) |
| `fill_uses` | Stratified values replaced by the fill value: empty values and, in permissive mode, every stratified field of a header with the wrong number of fields |

#### entropy

| Key | Meaning |
|---|---|
| `estimator` | The estimator, spelled with underscores |
| `log_base` | 2: entropies are in bits |
| `rarefaction` | For the `dima` estimator, the constants of [the paper's correction](/explanation/entropy/#the-papers-correction-fig-2); null for the other estimators. `grid` `paper_fig2`: subsample percentages from the start to 95 in steps of 5 (DiMA v5.0.9's step), plus 100%, the full data set. `start_rounding` `ceil_to_5`: the grid starts at 100·T/N rounded up to a whole number and then to a multiple of 5, and at 50 at least. `size_rounding` `ceil`: a subsample of R% of N sequences has R·N/100 sequences, rounded up. `sampling` `expected_with_replacement`: the [exact average](/reference/glossary/#exact-expectation) over all with-replacement subsamples, not one random draw. `min_points` 5: with fewer regression points — support below 1.25 × T, rounded up — a position keeps its observed entropy (`too_few_points`); DiMA v5.0.9's minimum, which the paper does not state |

## Conserved sequences

Written by `--hcs-output` and `dima hcs`. See
[Historically conserved sequences](/explanation/conserved-sequences/).

| Key | Meaning |
|---|---|
| `schema_version` | `"1.0.0"` |
| `tool` | `{"name": "dima", "version": …}`: the version that computed the regions |
| `source` | The results the regions were computed from: `query_name`, `kmer_length`, `support_threshold`, `sequence_count`, `input_checksum` (as in `run.input`), `tool_version` and `results_schema_version`. `dima cite --from` compares it with the results it is given, so it never mixes two runs |
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
| `conservation_lower_bound_95` | number | The lowest per-position exact 95% Clopper–Pearson lower bound among the members (proportion); not a bound on the share of sequences that carry the whole region (see [what a region's numbers mean](/explanation/conserved-sequences/#what-a-regions-numbers-mean)) |
| `low_support_positions` | array of integers | Every position from the first to the last member with support ≤ $T$, member or not |
| `flagged` | boolean | `low_support_positions` is not empty |
| `split_from_previous` | boolean | The region's first k-mer overlaps the previous region and disagrees with it on a shared column, which ended that region; the two regions then share columns |
| `members` | array | The qualifying positions, in order: `position`, `kmer` (the Index k-mer), `count`, `support`, `incidence` and `conservation_lower_bound_95` |

The positions listed in `low_support_positions` run from the first member to the last, not to the
end of the region's columns: region 2 of the repository's tiny test results
(`tests/golden/tiny/results.json`, `dima hcs` at 100%) spans columns 7–17 with members 7–15 and
lists `[12, 13, 14, 15]`; positions 16 and 17, without support, lie past its last member and are
not listed.

Regions can share columns: a region `split_from_previous` starts inside the previous one. On the
MERS-CoV spike gene at 95% (`dima hcs --hcs-min-incidence 95`), regions 34 (columns 3473–3543),
35 (3518–3572, flagged, support 2) and 36 (3547–3579) overlap, 35 and 36 being split. Adding up
`length` then counts shared columns twice: the 39 regions' lengths sum to 3,861 columns, but they
cover 3,809 distinct ones.

## Run report

Written by `--run-report PATH` (and the desktop app's *Save run report…*), also when the run fails
or is interrupted. It describes how the run went and is kept separate from the results, which
stay byte-identical from run to run. It never contains host or user names, directories, the raw
command line or environment values.

It is always JSON, always indented, whatever `--pretty` says. A name ending in `.json.gz` or
`.json.zst` gives a compressed report; a name that would mean a table or JSON Lines (`.tsv`,
`.jsonl`) is refused. `--run-report -` writes it to standard output, which is possible only when
the results go to a file (`-o FILE`). See
[keep a run report](/how-to/reproducibility/#keep-a-run-report).

| Key | Meaning |
|---|---|
| `schema_version` | `"1.1.0"` (1.1.0 added `threads.started`) |
| `tool` | `name` (`dima` or `dima-gui`), `version` and `target` (the Rust target triple of the binary, e.g. `x86_64-unknown-linux-gnu`) |
| `command` | The equivalent command, rebuilt from the parsed options: long option names, file names without directories |
| `status`, `exit_code`, `error` | `completed`, `failed` or `cancelled`; the [exit code](/reference/exit-codes/); the error report with directories removed (null unless failed) |
| `start_time`, `end_time` | UTC, RFC 3339 with milliseconds |
| `threads` | `count` requested, `started` (the same, or fewer when the system refused some; null if the run stopped before starting them) and `source`: `--threads`, `RAYON_NUM_THREADS`, `available_parallelism` (the CPUs the process may use, "available CPUs" in the `-v` output), `settings` (the desktop app's **Worker threads**), or the name of the scheduler variable that limited the count, such as `SLURM_CPUS_PER_TASK`, `PBS_NUM_PPN`, `NSLOTS` or `LSB_DJOB_NUMPROC` — the name only, never its value; null if the run stopped earlier |
| `platform` | `os` (`linux`, `macos`, `windows`, …), `arch` (`x86_64`, `aarch64`, …) and `logical_cpus` (logical CPUs available to the process) |
| `resources` | `wall_seconds` (wall-clock time of the run), `cpu_user_seconds`, `cpu_system_seconds` and `peak_rss_bytes` (null where not measured, as in the desktop app), `waited_for_input_seconds` (time the counting threads waited for the reader while counting k-mers; the first batch, read while opening, is not included; null when the analysis itself did not finish — a run that fails or is cancelled while its outputs are written keeps it) |
| `phases` | Wall time of each phase reached, in order (below); for a run that failed or was cancelled, the phases it had reached, the last one until it stopped |
| `work` | `records` read (after a failure, those read until it, which can be 0) and `positions` analysed (null when the analysis did not finish) |
| `inputs`, `outputs` | Each file's `role` (`alignment`, `results`, `results_jsonl`, `positions_table`, `variants_table`, `metadata_table`, `hcs_table`, `hcs`), `name`, `size_bytes` and `sha256` of its stored bytes. The input is listed when it was read to the end; the outputs are those completed, so after a failure or a cancellation only the ones finished before it. The run report itself is never listed |

The phases are those that `-v` reports, under these names:

| `phases[].name` | `-v` | What it covers |
|---|---|---|
| `opening` | opening | opening the input, reading its first batch and detecting the alphabet |
| `counting` | counting k-mers | reading the rest of the input and counting its k-mers |
| `finalising` | computing statistics | the entropies, motifs and summaries of every position |
| `writing` | writing | computing conserved sequences and writing every output |
