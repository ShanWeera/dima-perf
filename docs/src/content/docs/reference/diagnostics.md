---
title: Diagnostics
description: Every diagnostic code DiMA can report — what it means, when it is raised, what its count counts and what to do — and how diagnostics are printed, recorded and shown in the desktop app.
---

A **diagnostic** reports a condition that did not stop the analysis but that you should know
about. Each condition is reported **once**, with the number of occurrences, up to five examples in
input order and, where something can be done, a hint — never as a flood of per-record messages.

- **In the results**, diagnostics are listed in `run.diagnostics` (and, for conserved sequences, in
  the `diagnostics` of the conserved-sequences file) as
  `{code, severity, message, count, examples, hint}`; see [JSON outputs](/reference/json/#run).
- **On standard error**, a warning is printed as `warning[code]: message`, followed by an `e.g.`
  line with up to three of its examples and a `hint:` line. When more examples are stored than are
  shown (up to five are kept), the `e.g.` line ends with `; and more`; the full count is in the
  JSON. Informational diagnostics are printed the same way as `note[code]:`, with `-v` only. `-q`
  hides both.
- **In the desktop app**, the top bar shows a badge: "⚠ 1 warning" (or "⚠ N warnings") when there
  are warnings, "N notes" when there are only informational diagnostics, and "No warnings"
  otherwise; the notes are not counted while there are warnings. Clicking it opens the
  Diagnostics window, which lists every condition with *Warning* or *Note*, its code, ×N for its
  count, every stored example and its *Hint:*, or "No conditions were reported for this run.".
  The hints use command-line names; [In the desktop app](#in-the-desktop-app) gives the control
  each one means.

**Severity.** A `warning` is something that may affect how you read the results: look at it
before you use them. An `info` diagnostic (shown as a note) records something DiMA did on your
behalf, such as reading lower-case letters as upper case; nothing needs doing.

**Stability.** A code's name never changes within a schema major version. A new release may add
codes without changing the schema version, because `code` is a free string: DiMA 1.2.0 added
[`uncorrected_eligible_positions`](#uncorrected_eligible_positions) under results schema 1.1.0. A
reader that does not know a code keeps it as it is.

## Codes

The *count* column says what `count` counts.

| Code | Severity | Raised when | Count |
|---|---|---|---|
| [`lowercase_folded`](#lowercase_folded) | info | lower-case letters were read as upper case | letters |
| [`t_u_folded`](#t_u_folded) | info | U was read as T (DNA) or T as U (RNA) | letters |
| [`alphabet_contradiction`](#alphabet_contradiction) | warning | `--alphabet` disagrees with the residues | 1 |
| [`interior_whitespace`](#interior_whitespace) | warning | white space between residues was ignored | sequence lines |
| [`unknown_characters_excluded`](#unknown_characters_excluded) | warning | characters outside the alphabet were excluded | characters |
| [`header_field_count_mismatch`](#header_field_count_mismatch) | warning | headers do not have the fields of `--header-format` | headers |
| [`header_utf8_replaced`](#header_utf8_replaced) | warning | invalid UTF-8 in headers was replaced | invalid UTF-8 sequences |
| [`header_values_cleaned`](#header_values_cleaned) | info | header values were trimmed, cleaned or filled | changes (see below) |
| [`near_duplicate_values`](#near_duplicate_values) | warning | values of a field differ only in case, spacing or character forms | groups of values |
| [`missing_like_values`](#missing_like_values) | warning | values that usually mean "missing" | records |
| [`identifier_like_field`](#identifier_like_field) | warning | a stratified field has a different value in every record | fields |
| [`threshold_exceeds_sequence_count`](#threshold_exceeds_sequence_count) | warning | $T$ is not below the number of sequences | 1 |
| [`no_eligible_positions`](#no_eligible_positions) | warning | no position has support above $T$ | 1 |
| [`uncorrected_eligible_positions`](#uncorrected_eligible_positions) | warning | eligible positions kept their observed entropy | positions with more than one k-mer |
| [`below_min_coverage`](#below_min_coverage) | warning | positions have sample coverage below `--min-coverage` | positions |
| [`hcs_low_support_regions`](#hcs_low_support_regions) | warning | conserved regions include low-support positions | regions |
| [`flag_has_no_effect`](#flag_has_no_effect) | warning | an option has no effect with the others | 1 |

### lowercase_folded

Lower-case letters in sequence lines were read as upper case (soft-masked or lower-case
alignments). The count is also in `run.validation.lowercase_folded`. Nothing to do.

### t_u_folded

The alignment contains U in a DNA alignment (read as T) or T in an RNA alignment (read as U). The
count is in `run.alphabet.t_u_folded`. Nothing to do, unless the alphabet is wrong.

### alphabet_contradiction

An explicit `--alphabet` differs from what detection finds in the first records ("`--alphabet
protein` was given, but the residue composition of the first 191 records looks like dna"). The
analysis uses your choice, and letters that are not residues of it are excluded or rejected. The
message says it all, so there is no example. *Hint:* check `--alphabet`.

`--alphabet protein` on a nucleotide alignment runs to the end with this warning, because every
nucleotide letter is also a protein letter. `--alphabet dna` or `rna` on a protein alignment
stops instead in strict mode (the default), at the first amino-acid letter, before the warning is
recorded; the error's hint then names the cause first: "check --alphabet first: the residues of
the first records look like protein, not dna; or use --validation permissive …". *Fix:* give the
right `--alphabet`, or leave it out to detect it; permissive mode would exclude every k-mer with
such a letter.

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

When **no** header has the format's number of fields, the format is the likely mistake, and both
the warning and the strict-mode error say so first: "no header has the 4 fields of
--header-format, so the format is probably wrong: check it against the headers". *Fix:* compare
the format with a header (`grep -m 1 '>' alignment.fasta`) and correct it.

### header_utf8_replaced

With `--validation permissive` or `report`: headers contained bytes that are not valid UTF-8, and
each invalid sequence was replaced by U+FFFD. In strict mode the run stops at the first one.
Headers are checked only with `--header-format`. *Hint:* convert the file to UTF-8, for example
with `iconv`.

### header_values_cleaned

Header values were changed by the [cleaning rules](/explanation/input/#header-metadata): values
trimmed, invisible characters removed (examples list them by code point), values changed by NFC
normalisation, empty or mismatched values replaced by the fill value. Its count adds the four:
on the MERS-CoV spike sample stratified by host, country and year, 189 trimmed values and 1 filled
value give 190. Details are in `run.validation.header_cleaning`. Trailing spaces in headers are
common, so this is information only.

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
They are kept as ordinary values; only empty values receive the fill value. A value equal to the
fill value is never reported, so `--header-fillna NA` keeps `NA` quiet. Examples give each value
with its number of records. *Hint:* if they mean missing, empty them in the headers so they
receive the fill value, or edit them consistently.

### identifier_like_field

A stratified field had a different value in every one of the first records examined (at least 100
records are needed), like an accession number or a strain name. Such a field adds a metadata
entry for every sequence to every variant, which makes the results very large and is rarely
meant. *Hint:* the exact `--metadata-fields` option that stratifies by the other fields; when every
stratified field is identifier-like, "every stratified field is identifier-like; omit
--header-format if no stratification is needed". See
[Stratify by header metadata](/how-to/metadata/#identifier-like-fields).

### threshold_exceeds_sequence_count

The support threshold is not below the number of sequences, so no position can have support above
it: every position is NS, LS or ELS and the summaries are `null`. What the entropies are depends on
the [estimator](/reference/glossary/#estimator):

- with `dima` (the default) or `raw`, every entropy is the observed value;
- with `chao-wang-jost` or `miller-madow`, the estimate still applies to every position with at
  least 2 sequences; only the summaries are empty.

The message says it all, so there is no example. The hint gives the largest threshold that helps:
for the 26 sequences of DiMA v5.0.9's test data with the default $T = 100$,

```text
warning[threshold_exceeds_sequence_count]: the support threshold (100) is not below the number of sequences (26), so no position can have support above it: the summaries are empty and every entropy is the observed value
  hint: lower --support-threshold to 20 or less, so that positions supported by all 26 sequences are eligible and corrected; or add sequences
```

With `dima` that largest threshold is 0.8 × N rounded down, because the paper's correction needs
support of at least 1.25 × T (see [`uncorrected_eligible_positions`](#uncorrected_eligible_positions));
with the other estimators it is N − 1. With one sequence no threshold helps, and the hint says to
add sequences. A shorter `-k` cannot help either: no position can have more support than there
are sequences.

### no_eligible_positions

There are more sequences than $T$, but no position has support above $T$: gaps, missing data or
ambiguity codes leave fewer than $T$ valid k-mers at every position. *Hint:* consider a lower
`--support-threshold` or a shorter `-k` — a shorter window spans fewer gaps and ambiguous
characters, so it can raise support here, unlike in `threshold_exceeds_sequence_count`.

### uncorrected_eligible_positions

With the paper's estimator (`dima`, the default) only. The paper's correction needs at least five
regression points (the full data set included; a minimum taken from DiMA v5.0.9, which the paper
does not state), so it is applied to an eligible position only when its support is at least
1.25 × T, rounded up — 125 sequences for $T = 100$. An eligible position with support between $T$
and that keeps its observed entropy (`entropy_method` `plug_in`, `entropy_fallback`
`too_few_points`), and so does one in the single tiny case $T = 2$ with support 3, where every
subsample has the same size (`degenerate_regression`). Such positions are still eligible and are
included in the summaries; this warning says how many there are.

Its count is the positions with **more than one** k-mer. A position with a single k-mer has 0 bits
either way, because the correction would also give 0; those are named in the message but not
counted, and they alone raise no warning. The examples give each position's support and number
of distinct k-mers. Other estimators never raise it: `chao-wang-jost` and `miller-madow` correct
every position with at least 2 sequences, and `raw` corrects none.

**Example.** The HIV-1 *pol* protein alignment of the repository
(`tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2`), analysed with the defaults:

```sh
dima analyze -i HIV1_ALL_2020_pol_PRO.fasta.bz2 -o hiv.json
```

```text
warning[uncorrected_eligible_positions]: 2 of the 476 eligible positions kept their observed entropy: the paper's correction needs at least five regression points (a minimum taken from DiMA v5.0.9), which support below 125 (1.25 × T) does not give; they are included in the summaries
  e.g. position 42 (support 105, 38 distinct k-mers); position 43 (support 105, 33 distinct k-mers)
  hint: to have them corrected, raise their support to at least 125 (more sequences with a valid k-mer there) or lower --support-threshold to at most 84, and report the threshold you used; entropy_fallback marks them
analysed 3756 sequences, 1189 positions (476 eligible) in 0.17 s; entropy mean 1.9072, min 0.0768, max 5.9658 bits; 1 warning (see above)
```

The two positions are supported by 105 sequences, above $T = 100$ but below 125:

```sh
jq -c '.results[41,42] | {position, support, entropy, observed_entropy, entropy_method, entropy_fallback}' hiv.json
```

```text
{"position":42,"support":105,"entropy":4.004131590360346,"observed_entropy":4.004131590360346,"entropy_method":"plug_in","entropy_fallback":"too_few_points"}
{"position":43,"support":105,"entropy":3.5039311113741296,"observed_entropy":3.5039311113741296,"entropy_method":"plug_in","entropy_fallback":"too_few_points"}
```

When **no** eligible position was corrected, the message says so. The 26 sequences of DiMA
v5.0.9's test data with `-s 25` give "none of the 997 eligible positions was corrected: …
support below 32 (1.25 × T) does not give; they kept their observed entropy and are included in
the summaries; 916 of them have one k-mer, where the correction would also give 0 bits" (count
81); with `-s 20`, every one of the 1,031 eligible positions is corrected and there is no
warning. To see how a run's eligible positions were treated:

```sh
jq -c '[.results[] | select(.low_support == null) | .entropy_fallback // "corrected"] | group_by(.) | map({(.[0]): length}) | add' hiv.json
```

```text
{"corrected":474,"too_few_points":2}
```

*Hint:* the support those positions need, and the largest threshold that corrects them all
(0.8 × their lowest support, rounded down); a lower threshold also changes which positions are
eligible, so report the one you used. *What to do:* nothing, if a few positions keep their
observed entropy — the eligible mean then mixes corrected and observed values, which the warning
makes visible; to compare datasets on corrected values only, choose $T$ so that the positions you
compare are supported by at least 1.25 × T sequences. See
[the paper's correction](/explanation/entropy/#the-papers-correction-fig-2).

### below_min_coverage

With `--min-coverage X`: positions whose estimated sample coverage is below X. Each is marked
`below_min_coverage: true`; examples give positions and their coverage, shown so that a coverage
just below X never reads as X (`coverage > 0.9499` for 0.94996 with X = 0.95). *Hint:* their
diversity is incompletely sampled; see `sample_coverage` and
[sample coverage](/explanation/entropy/#sample-coverage).

### hcs_low_support_regions

In the conserved-sequence output: regions that include positions with support at or below $T$.
Examples give the region, its columns, its minimum support and its 95% lower bound (`> 99.9%`
rather than a rounded `100.0%`). *Hint:* see each region's `conservation_lower_bound_95`, or build
regions from positions with support above the threshold only with `--hcs-eligible-only`. See
[flagging](/explanation/conserved-sequences/#flagging-regions-that-rest-on-few-sequences).

### flag_has_no_effect

In the results: a fill value (`--header-fillna`) was given without `--header-format`. *Hint:*
name the header fields with `--header-format`.

## Warnings and notes of the command line

These are printed by `dima` on standard error and are not recorded in the results, which do not
depend on them. Warnings count in the summary line's "N warnings"; `-q` hides them.

| Code | Raised when |
|---|---|
| `flag_has_no_effect` | `--hcs-min-incidence` or `--hcs-eligible-only` without an output of conserved sequences (*fix:* add `--hcs-output PATH` or `--tsv-dir DIR`, or write the table with `-o PATH.tsv --table hcs`); `--pretty` without JSON output; `--similarity-weighted-entropy` without an output that holds its values, the results (JSON, JSONL) or the positions table: the values are then not computed (*fix:* write the results with `-o PATH.json`, or the positions table with `-o PATH.tsv --table positions` or `--tsv-dir DIR`) |
| `output_format_assumed` | a conserved-sequences file (`--hcs-output`, or `dima hcs -o`) is named without `.json` or `.tsv`, for example `regions.txt`: "'regions.txt' does not name JSON or TSV, so the conserved regions are written to it as JSON" (*fix:* name it `regions.json` or `regions.tsv`, optionally followed by `.gz` or `.zst`; with `dima hcs`, or choose the format with `-O json` or `-O tsv`) |
| `newer_schema` | `dima view` read results written with a newer (or an unrecognised 1.x) results schema version than its own, such as `1.2.0` or `1.1`; it writes them with its own version and keeps only the keys it knows. A version whose major number (before the first dot) is not 1 is refused instead (exit 4). *Fix:* to keep every key, convert the file with the DiMA version that wrote it (its `run.tool.version`) or a newer one |
| `threads_exceed_cpus` | more threads were requested (`--threads` or `RAYON_NUM_THREADS`) than CPUs are available; they are used anyway, and more threads than CPUs slows the run down. *Fix:* lower `--threads` to the number of CPUs available, or leave it out to use them all |
| `threads_refused` | the system refused to start some of the worker threads (a limit on processes and threads, such as `ulimit -u` or a container's pids limit, or a lack of memory), so the analysis continued with those it started; the run report records both numbers (`threads.count`, `threads.started`). Results are the same with any number of threads |

Two notes (`note:`, never counted as warnings) may appear too:

- `JSONL holds one position per line; provenance and summaries are in the JSON output`, from
  `dima analyze` and `dima view` when they write JSON Lines. Nothing needs doing; also write the
  JSON (`-o results.json`) if you need the summaries or the provenance.
- From `dima view`, when it writes results of an older 1.x schema, and only when its outputs hold
  per-position values (results JSON or JSONL, or the positions table): for example, results
  written by dima 1.0.0 have no similarity-weighted entropy, so those keys are written as `null`
  (`NA` in the positions table) and the option as off. Nothing needs doing; re-run the analysis
  with `--similarity-weighted-entropy` if you want the values.

## In the desktop app

The app shows the same diagnostics, with hints written in command-line names. Each name means
this control:

| In a hint | In the desktop app |
|---|---|
| `--support-threshold` | **Support threshold T** |
| `-k` | **k-mer length** |
| `--alphabet` | **Alphabet** |
| `--validation permissive` | **Advanced** ▸ **Validation** ▸ **Permissive** |
| `--header-format` | the field names of the header editor |
| `--metadata-fields` | the **Stratify** boxes of the header editor |
| `--header-fillna` | **Advanced** ▸ **Fill value** |
| `--min-coverage` | **Advanced** ▸ **Minimum coverage** ▸ **flag below** |
| `--estimator` | **Advanced** ▸ **Entropy estimator** |
| `--threads` | **Advanced** ▸ **Worker threads** |
| `--hcs-min-incidence` | **Conserved regions (HCS) at** … **Index incidence** |
| `--hcs-eligible-only` | **Eligible positions only** |
| `run.validation.characters` | the *Characters in sequence lines* table under *Input validation* in the Diagnostics window |
| `run.metadata.mismatches` | *Mismatched headers* under *Input validation* in the Diagnostics window |
| `entropy_fallback`, `sample_coverage` | the selected position's *Method* and *Sample coverage* facts |

The app's own notifications ("Warning:", "Note:" and "Done:" messages) and the command line's
`note:` lines are messages, not diagnostics: they are not recorded in the results.
