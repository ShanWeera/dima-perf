---
title: Diagnostics
description: Every diagnostic code DiMA can report — what it means, when it is raised and what to do — and how diagnostics are printed and recorded.
---

A **diagnostic** reports a condition that did not stop the analysis but that you should know
about. Each condition is reported **once**, with the number of occurrences, up to five examples in
input order and, where something can be done, a hint — never as a flood of per-record messages.

- In the results, diagnostics are listed in `run.diagnostics` (and, for conserved sequences, in
  the `diagnostics` of the HCS output) as `{code, severity, message, count, examples, hint}`;
  see [JSON outputs](/reference/json/#run).
- On standard error, warnings are printed as `warning[code]: message`, followed by up to three
  examples (`e.g.`) and the `hint:`; informational diagnostics are printed as `note[code]:` with
  `-v` only. `-q` hides both.
- The desktop app lists them in its diagnostics panel.

Codes are stable within a schema major version; a minor version may add codes.

## Codes

| Code | Severity | Raised when |
|---|---|---|
| [`lowercase_folded`](#lowercase_folded) | info | lower-case letters were read as upper case |
| [`t_u_folded`](#t_u_folded) | info | U was read as T (DNA) or T as U (RNA) |
| [`alphabet_contradiction`](#alphabet_contradiction) | warning | `--alphabet` disagrees with the residues |
| [`interior_whitespace`](#interior_whitespace) | warning | white space between residues was ignored |
| [`unknown_characters_excluded`](#unknown_characters_excluded) | warning | characters outside the alphabet were excluded |
| [`header_field_count_mismatch`](#header_field_count_mismatch) | warning | headers do not have the fields of `--header-format` |
| [`header_utf8_replaced`](#header_utf8_replaced) | warning | invalid UTF-8 in headers was replaced |
| [`header_values_cleaned`](#header_values_cleaned) | info | header values were trimmed, cleaned or filled |
| [`near_duplicate_values`](#near_duplicate_values) | warning | values of a field differ only in case, spacing or character forms |
| [`missing_like_values`](#missing_like_values) | warning | values that usually mean "missing" |
| [`identifier_like_field`](#identifier_like_field) | warning | a stratified field has a different value in every record |
| [`threshold_exceeds_sequence_count`](#threshold_exceeds_sequence_count) | warning | $T$ is not below the number of sequences |
| [`no_eligible_positions`](#no_eligible_positions) | warning | no position has support above $T$ |
| [`below_min_coverage`](#below_min_coverage) | warning | positions have sample coverage below `--min-coverage` |
| [`hcs_low_support_regions`](#hcs_low_support_regions) | warning | conserved regions include low-support positions |
| [`flag_has_no_effect`](#flag_has_no_effect) | warning | an option has no effect with the others |

### lowercase_folded

Lower-case letters in sequence lines were read as upper case (soft-masked or lower-case
alignments). The count is also in `run.validation.lowercase_folded`. Nothing to do.

### t_u_folded

The alignment contains U in a DNA alignment (read as T) or T in an RNA alignment (read as U). The
count is in `run.alphabet.t_u_folded`. Nothing to do, unless the alphabet is wrong.

### alphabet_contradiction

An explicit `--alphabet` differs from what detection finds in the first records ("`--alphabet
protein` was given, but the residue composition of the first 191 records looks like dna"). The
analysis uses your choice, and letters that are not residues of it are excluded or rejected.
*Hint:* check `--alphabet`.

### interior_whitespace

Sequence lines contain spaces or tabs between residues; they were ignored. Examples give the
record and line. This usually means the file was reformatted by a word processor or a
spreadsheet. *Hint:* check that the alignment file was not reformatted.

### unknown_characters_excluded

With `--validation permissive` or `report`: characters that belong to no class of the alphabet
(digits, other punctuation, letters outside the alphabet) were found, and the k-mers containing
them were excluded. Examples give the character, record, line and column. In strict mode the
first such character stops the run instead. *Hint:* `run.validation.characters` lists every
character; check `--alphabet`. See [Validate input characters](/how-to/validation/).

### header_field_count_mismatch

With `--validation permissive` or `report`: headers whose number of `|`-separated fields differs
from `--header-format`. Their stratified fields received the fill value. Examples give the record,
line and number of fields; `run.metadata.mismatches` lists the first 20 headers in full. In strict
mode the run stops instead, listing the same headers. *Hint:* see `run.metadata.mismatches`.

### header_utf8_replaced

With `--validation permissive` or `report`: headers contained bytes that are not valid UTF-8, and
each invalid sequence was replaced by U+FFFD. In strict mode the run stops at the first one.
*Hint:* convert the file to UTF-8, for example with `iconv`.

### header_values_cleaned

Header values were changed by the [cleaning rules](/explanation/input/#header-metadata): values
trimmed, invisible characters removed (examples list them by code point), values changed by NFC
normalisation, empty or mismatched values replaced by the fill value. Details are in
`run.validation.header_cleaning`. Trailing spaces in headers are common, so this is information
only.

### near_duplicate_values

Within one stratified field, groups of distinct values that become equal when case is ignored,
runs of white space are collapsed and compatibility characters are folded (Unicode NFKC) — for
example `Saudi Arabia` and `saudi  arabia`. They are counted as different values. Examples list
each group with counts. *Hint:* make the spellings consistent in the headers if they mean the
same.

### missing_like_values

Within one stratified field, values that usually mean "missing" (compared without regard to case):
`missing`, `not applicable`, `not collected`, `not provided`, `restricted access` (the INSDC
missing-value terms), anything starting with `missing:`, and `NA`, `N/A`, `NaN`, `null`, `None`.
They are kept as ordinary values; only empty values receive the fill value. Examples give each
value with its number of records. *Hint:* if they mean missing, empty them in the headers so they
receive the fill value, or make them consistent.

### identifier_like_field

A stratified field had a different value in every one of the first records examined (at least 100
records are needed), like an accession number or a strain name. Such a field adds a metadata
entry for every sequence to every variant, which makes the results very large and is rarely
meant. *Hint:* the exact `--metadata-fields` option that stratifies by the other fields. See
[Stratify by header metadata](/how-to/metadata/#identifier-like-fields).

### threshold_exceeds_sequence_count

The support threshold is not below the number of sequences, so no position can have support
above it: every position is NS, LS or ELS, every entropy is the observed value and the summaries
are `null`. *Hint:* lower `--support-threshold`, or add sequences.

### no_eligible_positions

There are more sequences than $T$, but no position has support above $T$ (gaps or ambiguity
codes reduce support everywhere). *Hint:* consider a lower `--support-threshold` or a shorter
`-k`.

### below_min_coverage

With `--min-coverage X`: positions whose estimated sample coverage is below X. Each is marked
`below_min_coverage: true`; examples give positions. *Hint:* their diversity is incompletely
sampled; see `sample_coverage` and [sample coverage](/explanation/entropy/#sample-coverage).

### hcs_low_support_regions

In the conserved-sequence output: regions that include positions with support at or below $T$.
Examples give the region, its columns, its minimum support and its 95% lower bound. *Hint:* see
each region's `conservation_lower_bound_95`, or build regions from eligible positions only with
`--hcs-eligible-only`. See [flagging](/explanation/conserved-sequences/#flagging-regions-that-rest-on-few-sequences).

### flag_has_no_effect

In the results: a fill value (`--header-fillna`) was given without `--header-format`. *Hint:*
name the header fields with `--header-format`.

## Warnings of the command line

These are printed by `dima` on standard error and are not recorded in the results, which do not
depend on them:

| Code | Raised when |
|---|---|
| `flag_has_no_effect` | `--hcs-min-incidence` or `--hcs-eligible-only` without an output of conserved sequences (*fix:* add `--hcs-output PATH` or `--tsv-dir DIR`, or write the table with `-o PATH.tsv --table hcs`); `--pretty` without JSON output; `--similarity-weighted-entropy` without an output that holds its values, the results (JSON, JSONL) or the positions table: the values are then not computed (*fix:* write the results with `-o PATH.json`, or the positions table with `-o PATH.tsv --table positions` or `--tsv-dir DIR`) |
| `newer_schema` | `dima view` read results written with a newer (or an unrecognised 1.x) results schema version than its own, such as `1.2.0` or `1.1`; it writes them with its own version and keeps only the keys it knows. A version whose major number (before the first dot) is not 1 is refused instead (exit 4). *Fix:* to keep every key, convert the file with the DiMA version that wrote it (its `run.tool.version`) or a newer one |
| `threads_exceed_cpus` | more threads were requested (`--threads` or `RAYON_NUM_THREADS`) than CPUs are available; they are used anyway |
| `threads_refused` | the system refused to start some of the worker threads (a limit on processes and threads, such as `ulimit -u` or a container's pids limit, or a lack of memory), so the analysis continued with those it started; the run report records both numbers (`threads.count`, `threads.started`). Results are the same with any number of threads |

`dima view` also prints a `note:` (never counted as a warning) when it writes results of an
older 1.x schema, and only when its outputs hold per-position values (results JSON or JSONL, or
the positions table): for example, results written by dima 1.0.0 have no similarity-weighted
entropy, so those keys are written as `null` (`NA` in the positions table) and the option as
off. Nothing needs doing; re-run the analysis with `--similarity-weighted-entropy` if you want
the values.
