---
title: Glossary
description: The terms used in DiMA's results, options, desktop app and documentation, each defined in a few sentences with every name it goes by and a link to the page that explains it.
---

Each term is defined briefly here and in full on the page it links to. Names in `code` are the
fields of the [JSON](/reference/json/) and [TSV](/reference/tsv/) outputs or the options of the
[command line](/reference/cli/); names in *italics* or "quotes" are what the desktop app shows.
Where one idea goes by several names, every name is listed under one entry.

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
(default 9 for proteins and 27 for nucleotides; the desktop app's *k-mer length*). See
[choose the k-mer length](/how-to/analyse/#choose-the-k-mer-length).

### Valid k-mer

A k-mer whose characters are all residues. A gap, missing-data symbol, ambiguity code, stop or
frameshift symbol anywhere in the window makes it invalid, and that sequence then does not count
at that position. See [valid k-mers and support](/explanation/method/#valid-k-mers-and-support).

### Variant

A distinct valid k-mer at a position, with the number of sequences that carry it. In the results
a variant is an entry of `diversity_motifs`, and its k-mer is its `sequence`; a member of a
conserved region names its k-mer `kmer`. The desktop app's variant table calls the column
*Sequence* and offers *Copy k-mer*.

### Distinct k-mers (D)

The number D of different valid k-mers at a position, the Index included: 1 at a fully conserved
position, 0 without support. `distinct_variants_count` is a different number: it leaves out the
Index variant (or variants, when several are tied), so at MERS-CoV spike position 25 D is 10 and
`distinct_variants_count` is 9. The desktop app's *Distinct variants* fact shows
`distinct_variants_count` with the [distinct variants incidence](#distinct-variants-incidence):
"9 (21.95%)" at position 25.

### Alphabet

Protein, DNA or RNA: it decides which letters are residues. `--alphabet auto` (the default; the
desktop app's *Auto*) detects it from the residues of the first records with the rules of
`esl_abc_GuessAlphabet` in Easel, the sequence library of HMMER; `--alphabet protein`, `dna` or
`rna` sets it. The decision is recorded in
`run.alphabet` (`method` `detected` or `user`). See [the alphabet](/explanation/input/#the-alphabet).

### Residue, gap, ambiguity code, stop, frameshift and unknown character

The classes of the characters in sequence lines. A **residue** is a letter of the alphabet (A, C,
G, T for DNA; the 20 standard amino acids, U and O for proteins); a **gap** (`-` `.` `_`) marks a
column the sequence does not cover; a **missing-data** symbol (`~` `?`) marks an unknown stretch;
an **ambiguity code** stands for several residues (protein `X B J Z`; nucleotide `R Y K M S W B D
H V N X I`); a **stop** (`*` `$` `^`) and a **frameshift** (`#`) come from translated or annotated
alignments; **white space** is ignored; any other byte is **unknown** and is handled by the
[validation mode](#validation-mode). Only residues make up valid k-mers. See the
[symbol tables](/reference/symbols/).

### Symbol class

The class of one character byte in the alphabet used: `residue`, `gap`, `missing`, `ambiguity`,
`stop`, `frameshift`, `whitespace` or `unknown` (the `class` of each entry of
`run.validation.characters`, and the *Class* column of the desktop app's characters table). Not
to be confused with the [support class](#support-class) of a position.

## Support

### Support

The number of sequences with a valid k-mer at a position (N, `support`).

### Support threshold

The value T that each position's support is compared with (`-s`, `--support-threshold`, default
100, the paper's value; the desktop app's *Support threshold T*). See
[the support threshold](/explanation/method/#the-support-threshold-and-the-low-support-labels).

### Support class

Where a position's support stands against T: NS (support 0), LS (below T), ELS (equal to T) or
eligible (above T). It is the `low_support` key (null for eligible positions) and the desktop
app's *Class* column (✔ for eligible). Not to be confused with the [symbol class](#symbol-class)
of a character.

### Eligible position

A position with support above the support threshold. Eligible positions enter the summary
statistics. With the paper's estimator (`dima`, the default) an eligible position's entropy is
[corrected](#corrected-entropy) *where possible*; positions with support at or below T keep their
observed entropy.

### Eligible mean

The average entropy of the eligible positions: their entropies added up and divided by how many
there are, each position counting once whatever its support. It is one number for the whole
alignment, and it uses the run's [reported entropy](#entropy-reported-entropy) (`entropy`: the
corrected entropy where possible, by default). It appears as `average_entropy` in the results,
`entropy mean` in the [summary line](#summary-line) of `dima analyze`, the *mean* chip of the
desktop app and, on the app's entropy chart, the dashed grey line labelled *eligible mean*: 0.1261
bits for the MERS-CoV spike sample (`samples/mers_spike_aa.fasta` in the repository). Positions
with support at or below the threshold are not in it, and it is `null` (— in the app) when no
position is eligible. When the app's chart shows similarity-weighted entropy, the same line is the
eligible mean of similarity-weighted entropy (`similarity_weighted_entropy_summary.average_entropy`,
the first number of the app's similarity-weighted chip). See
[the eligible mean](/explanation/motifs/#the-eligible-mean).

### NS, LS and ELS

The labels (`low_support`) of positions that are not eligible: NS, no support (N = 0; the desktop
app's chart labels these positions "no usable support"); LS, low support (N below T); ELS, support
equal to the threshold (N = T). LS on its own is the narrow sense of "low support"; for the wider
sense see [Low support](#low-support).

### Low support

The positions with support at or below T. In most names it is the umbrella of all three labels,
NS + LS + ELS: the `low_support` key, `low_support_count`, a conserved region's
`low_support_positions`, the warning `hcs_low_support_regions` and a region "flagged for low
support", the desktop app's *low support* chip ("low support 713 (NS 551, LS 162, ELS 0)") and its
filter *Low support (NS, LS, ELS)*. On the app's entropy chart, the dashed line and the "(low
support)" mark of the normalised values cover LS and ELS only, because NS positions have no value
to draw.

### Low-support count

`low_support_count`: the number of NS, LS and ELS positions together, as DiMA v5.0.9 counted it.
The counts per label are in `support_label_counts`.

## Entropy

### Entropy (reported entropy)

The entropy a run reports for a position, in bits (`entropy`): the value of the run's
[estimator](#estimator) — by default the [corrected entropy](#corrected-entropy) where it can be
computed and the [observed entropy](#observed-entropy) elsewhere. It is `null` (`NA`) without
support. The desktop app calls it *Entropy* (table and facts), its exported figures "Shannon entropy
(bits)", and the summary line averages it as `entropy mean`. `entropy_method` says which method
produced it.

### Observed entropy

The Shannon entropy, in bits, of a position's variant counts as observed (`observed_entropy`),
also called the plug-in, raw or uncorrected entropy; the method name is `plug_in` and the
estimator that reports it everywhere is `raw`. It underestimates the entropy of the population,
more so with fewer sequences. See
[observed entropy](/explanation/entropy/#observed-plug-in-entropy).

### Corrected entropy

The paper's sample-size correction of the entropy (`entropy_method` `dima`), the default. With the
paper's estimator an eligible position's entropy is corrected *where possible*: the correction
needs at least five regression points (the full data set included; a minimum taken from DiMA
v5.0.9, which the paper does not state), so a position is corrected only when its support is at
least 1.25 × T, rounded up (125 sequences for T = 100). With support between T and that, and in
the one tiny case T = 2 with support 3, it keeps its observed entropy
(`too_few_points`, `degenerate_regression`). Such positions are still eligible and are included in
the summaries; the warning
[`uncorrected_eligible_positions`](/reference/diagnostics/#uncorrected_eligible_positions) counts
those with more than one k-mer. See
[the paper's correction](/explanation/entropy/#the-papers-correction-fig-2).

### Estimator

The method behind the reported entropy (`--estimator`): `dima` (the paper's correction, the
default), `chao-wang-jost`, `miller-madow` or `raw`. The results spell them with underscores
(`chao_wang_jost`, `miller_madow` in `run.parameters.estimator`), and the desktop app names them
*DiMA (paper, Fig. 2)*, *Chao–Wang–Jost*, *Miller–Madow* and *Raw (plug-in)*. Each position records
the method used ([entropy method](#entropy-method)). See
[alternative estimators](/explanation/entropy/#alternative-estimators).

### Entropy method

The method that produced a position's reported entropy (`entropy_method`): `dima` (the paper's
correction), `chao_wang_jost`, `miller_madow`, `plug_in` (the observed entropy: with `raw`, or
when the requested estimator fell back) or `not_estimable` (no support, no entropy).

### Entropy fallback

Why a position's entropy was computed with another method than the one requested
(`entropy_fallback`; the desktop app writes the values with spaces, "too few points"):

- `support_at_or_below_threshold`: with `dima`, the position is not eligible (support ≤ T);
- `too_few_points`: with `dima`, an eligible position whose support is below 1.25 × T, rounded up,
  which gives the correction fewer than five regression points;
- `degenerate_regression`: with `dima`, T = 2 and support 3, where every subsample has the same
  size;
- `single_observation`: with `chao-wang-jost` or `miller-madow`, support 1.

It is `null` when the requested estimator was used, with `raw`, and without support.

### Rarefaction

Computing a value on subsamples of decreasing size. The paper's correction computes the entropy of
subsamples of R% of a position's sequences — R from 50, or from 100·T/N rounded up to a multiple
of 5, to 95, plus the full data set — and extrapolates it to an infinite sample; its constants are
recorded in `run.entropy.rarefaction` (`null` with the other estimators). Rarefying two samples to
the same size in order to compare them is another use of the word, which DiMA does not do. See
[the paper's correction](/explanation/entropy/#the-papers-correction-fig-2).

### Exact expectation

How DiMA evaluates the paper's correction: the paper draws one random subsample per grid point,
and DiMA computes instead the exact average of the result over all possible with-replacement
subsamples, so no random numbers are used and every run gives the same value (`sampling`
`expected_with_replacement`). See [the exact expectation](/explanation/entropy/#the-exact-expectation).

### Bit and nat

Units of entropy. A **bit** uses base-2 logarithms; DiMA reports every entropy in bits
(`log_base` 2). A **nat** uses natural logarithms; 1 nat = 1/ln 2 ≈ 1.4427 bits. Some formulas,
such as the Chao–Wang–Jost estimator's, are written in nats and converted.

### Pseudo-entropy

The paper's value of 0 for a position without support ("pseudo entropy", tagged NS). DiMA keeps the
NS tag but writes `null` (`NA`) instead, because a value that was not observed is missing, not
zero. See [positions without support](/explanation/entropy/#positions-without-support).

### Sample coverage

The estimated probability that one more sequence would carry a k-mer already seen at that position
(`sample_coverage`; the desktop app's *Sample coverage* fact and *Coverage* column). Low coverage
means that much of the diversity there has probably not been sampled. It is not sequencing depth.
It is `null` with support below 2, and exactly 1 where no k-mer is seen once, and also where
exactly one k-mer is seen once and none twice: the second factor of the formula on the entropy
page is then 0. On the MERS-CoV spike sample this second case gives coverage 1 at 394 positions,
for example position 3, whose two 9-mers are carried by 170 sequences and 1. See
[sample coverage](/explanation/entropy/#sample-coverage).

### Minimum coverage

The value given with `--min-coverage` (the desktop app's *flag below* checkbox under **Advanced**,
0.95 when ticked): positions whose sample coverage is below it are marked
`below_min_coverage: true` and reported by the warning `below_min_coverage`, and
`coverage_summary.below_min_count` counts them. It changes no value. "Flag" names two other things
as well: [flagged regions](#flagged-region) of conserved sequences, and the warning
`flag_has_no_effect` (an option that has no effect).

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

The motif of each variant of a position, with its short code (`motif_short`) and full name
(`motif_long`):

- **Index** (`I`): the most common variant, if its count is above 1; variants tied at that count
  are all Index;
- **Major** (`Ma`): the most common of the other variants seen more than once; variants tied at
  that count are all Major;
- **Minor** (`Mi`): every other variant seen more than once;
- **Unique** (`U`): a variant seen once. Where every variant is seen once there is no Index.

The list of a position's variants with their motifs is `diversity_motifs`, DiMA v5.0.9's name for
it. The desktop app's variant search also matches the codes. See
[motif classes](/explanation/motifs/#motif-classes).

### Top variant

The desktop app's *Top variant* column: the first variant of the position's list — the highest
count, ties in byte order of the k-mer — with its incidence, for example "SVKSACIEV  75.9%" at
MERS-CoV spike position 25. It is usually the Index; with tied Index variants it is the first of
them in byte order, and where every k-mer occurs once it is the first Unique variant in byte order
(HIV-1 *pol* protein position 29, support 4). The column sorts by the k-mer, alphabetically.

### Motif composition

The shares of a position's supporting sequences that carry Index, Major, Minor and Unique
variants, which the desktop app draws as a bar ("Index: 75.9% of the supporting sequences" at
MERS-CoV spike position 25). They are the incidences of the variants of each motif added up.

### Incidence

A variant's count as a percentage of the position's support (`incidence`).

### Total variants incidence

The percentage of a position's support carried by variants other than the Index
(`total_variants_incidence`): 24.12% at MERS-CoV spike position 25 (41 of 170 sequences). The
desktop app shows it in the *Variants %* column (one decimal), the *Total variants* fact and the
*total variants at N* chip of the selected position. It is 0 at a fully conserved position, and
also at a position whose variants are all tied Index variants. See
[incidences](/explanation/motifs/#incidences).

### Distinct variants incidence

The number of distinct non-Index variants as a percentage of the sequences that carry them
(`distinct_variants_incidence`): 21.95% at MERS-CoV spike position 25 (9 variants carried by 41
sequences). It is undefined (`null`) where no sequence carries a non-Index variant. See
[incidences](/explanation/motifs/#incidences).

### Relative incidence

The paper's share of a variant's sequences that carry one value of a stratified header field: the
value's count divided by the variant's count. At MERS-CoV spike position 25, 9 of the 14
sequences of the Major variant `SFKSACIEV` come from *Camelus dromedarius*: 64.3%, which the
desktop app's metadata panel shows as "64.3% (9)". See
[stratify by header metadata](/how-to/metadata/).

## Summaries

### All-positions summary

`all_positions_summary`: the mean, lowest and highest entropy over every position, with positions
without support counted as 0 bits and low-support positions with their reported entropy — DiMA
v5.0.9's averaging, kept for comparison with older results. It is not the paper's summary, and its
lowest entropy can be an NS position: on the HIV-1 *pol* protein alignment it is position 30,
0 bits, one of 672 tied positions, 551 of them NS. See
[run summaries](/explanation/motifs/#run-summaries).

### Tied positions

`tied_position_count`: how many positions have exactly the value of a lowest or highest entropy.
The extremum names the first of them (`position`): the MERS-CoV spike sample's lowest entropy,
0 bits, is first reached at position 7 and shared by 455 positions, which the desktop app's chip
shows as "lowest 0.0000 at position 7, 455 positions tied".

### Summary line

The last line that `dima analyze` prints on standard error, for example `analysed 191 sequences,
1345 positions (1345 eligible) in 0.03 s; entropy mean 0.1261, min 0.0000, max 1.5878 bits`;
`dima hcs` prints its own (`44 conserved regions with an Index incidence of at least 100%`).
These pages call it the summary line (also known as the one-line summary or the end-of-run
line). When warnings were printed above it, it ends with their number ("1 warning (see above)");
`-q` hides it. See [analyse an alignment](/how-to/analyse/).

## Conserved sequences

### Historically conserved sequence (HCS)

A region of consecutive positions whose Index variant reaches the threshold incidence
(`--hcs-min-incidence`, default 100%); also called a **conserved region** (the desktop app's
strip, and the `regions` of the conserved-sequences file). A region is built from its member
positions, which reach the threshold; positions between them, whose k-mers it spans, need not.
See [historically conserved sequences](/explanation/conserved-sequences/) and
[what a region's numbers mean](/explanation/conserved-sequences/#what-a-regions-numbers-mean).

### HCS threshold

The minimum Index incidence θ a position needs to join a conserved region: `--hcs-min-incidence`
(default 100), `parameters.threshold_percent` in the conserved-sequences file, the HCS or
conserved-region threshold of these pages, and in the desktop app *Conserved regions (HCS) at …*
with its *Index incidence* slider and exact field. It must be above 50 and at most 100, with up to
9 decimals.

### Eligible positions only

`--hcs-eligible-only` (the desktop app's *Eligible positions only* switch; `eligibility`
`support_gt_threshold` in the conserved-sequences file, `all_positions` otherwise): only positions
with support above T may join a region, and a position with support ≤ T between two members ends
the region instead of being flagged. See
[eligible positions only](/explanation/conserved-sequences/#eligible-positions-only).

### Flagged region

A conserved region that includes positions with support at or below the support threshold, so that
its conservation rests on few sequences (`flagged`; "flagged for low support"). See
[flagging](/explanation/conserved-sequences/#flagging-regions-that-rest-on-few-sequences).

### Conservation lower bound

The exact 95% Clopper–Pearson lower confidence bound of a member position's Index proportion
(count / support, `conservation_lower_bound_95`): how conserved the position is at least, given
the number of sequences. A region's bound is the lowest per-position bound among its members; it
is not a bound on the share of sequences that carry the whole region, which can be much lower.
The "coverage" of such an interval (its coverage probability: how often it contains the true
proportion, at least 95% here) has nothing to do with [sample coverage](#sample-coverage). See
[a lower bound on conservation](/explanation/conserved-sequences/#a-lower-bound-on-conservation).

## Input and metadata

### Header format

The names of the `|`-separated fields of the FASTA headers (`--header-format`). Names are trimmed
and [NFC](#nfc)-normalised. See [describe the headers](/how-to/metadata/#describe-the-headers).

### Stratified field

A header field per whose values each variant's sequences are counted: `--metadata-fields`
(default: every field of `--header-format`), the *Stratify* boxes of the desktop app's header
editor, and `run.metadata.stratified` in the results. It is also called a counted field or a
metadata field. See [stratify by header metadata](/how-to/metadata/).

### Metadata fields

See [stratified field](#stratified-field).

### Fill value

The value that replaces an empty stratified value (`--header-fillna`, default `Unknown`; the
desktop app's *Fill value*). In permissive mode it also replaces every stratified field of a header
with the wrong number of fields. It is trimmed and NFC-normalised like the field names, and a
value equal to it is never reported as missing-like. Its uses are counted in `fill_uses`.

### Identifier-like field

A stratified field with a different value in every one of the first records examined, such as an
accession or strain name (warning `identifier_like_field`; at least 100 records are needed). It
adds one metadata entry per sequence to every variant. See
[identifier-like fields](/how-to/metadata/#identifier-like-fields).

### Missing-like value

A value that usually means "missing" — `NA`, `N/A`, `NaN`, `null`, `None`, `missing` (also
`missing:…`) or one of the [INSDC](#insdc) terms — compared without regard to case (warning
`missing_like_values`). DiMA keeps it as an ordinary value; only an empty value receives the fill
value. See [values that mean "missing"](/how-to/metadata/#values-that-mean-missing).

### Near-duplicate value

One of several values of a field that become equal when case is ignored, runs of white space are
collapsed and compatibility characters are folded (Unicode NFKC), such as `Saudi Arabia` and
`saudi  arabia` (warning `near_duplicate_values`). They are counted as different values. See
[near-duplicate spellings](/how-to/metadata/#near-duplicate-spellings).

### NFC

Unicode Normalization Form C, the composed form of text: the same visible character can be stored
as one code point or as several (a letter and a combining accent), and NFC turns both into one.
DiMA normalises header field names, values and the fill value to NFC, so visually identical
spellings count as one value; `nfc_changed` counts the values it changed. See
[header metadata](/explanation/input/#header-metadata).

### INSDC

The International Nucleotide Sequence Database Collaboration of DDBJ, ENA and GenBank. Its
missing-value terms — `missing`, `not applicable`, `not collected`, `not provided` and
`restricted access` — are among the [missing-like values](#missing-like-value) DiMA reports.

### Validation mode

What happens to characters outside the alphabet (`--validation`; the desktop app's *Validation*
under **Advanced**): `strict` stops the run, `permissive` excludes the k-mers that contain them,
and `report` also lists every character found. See [validate input characters](/how-to/validation/).

### Query name

The name of the analysed protein or gene in the results (`query_name`): `--query-name`, or the
input file name without its extensions; `Unknown Protein`, `Unknown DNA` or `Unknown RNA` for
standard input and other inputs without a file name. See
[the query name](/explanation/input/#the-query-name).

### BGZF

Blocked gzip, the gzip variant that `bgzip` and samtools write for indexed files. It is valid gzip,
so DiMA reads it as gzip (`run.input.compression` `gzip`).

## Results and outputs

### Diagnostic

A condition that did not stop the analysis but that you should know about, reported once with a
code, a count, examples and a hint (`run.diagnostics`). Its severity is `warning` (it may affect
how the results are read; `warning[code]:` on the command line, *Warning* in the desktop app's
Diagnostics window) or `info` (something DiMA did for you; `note[code]:` with `-v`, *Note* in the
app). The `note:` lines of `dima view` and the app's "Note:" notifications are messages, not
diagnostics. See [Diagnostics](/reference/diagnostics/).

### Provenance

The `run` object of the results: the version, every parameter and where it came from, the input's
checksum, what reading found, the diagnostics and the references, with nothing that varies between
runs. See [run](/reference/json/#run).

### Run report

An optional, separate JSON file (`--run-report`; the desktop app's *Save run report…*) that
records how a run went: times, threads, memory, platform and exit status, without user or host
names. See [keep a run report](/how-to/reproducibility/#keep-a-run-report).

### Schema version

The version of a JSON output's format (`schema_version`). Its minor number changes when keys or
values are added (results schema 1.1.0 added similarity-weighted entropy), its major number when
the meaning of existing fields changes. Diagnostic codes are free strings, so a new release may add
codes without changing it: DiMA 1.2.0 added `uncorrected_eligible_positions` under results schema
1.1.0. See [JSON outputs](/reference/json/).

### JSON Lines

The `jsonl` output (`-O jsonl`, or a name ending in `.jsonl` or `.ndjson`): one JSON object per
line, each a position of `results[]`, without the summaries and the provenance, which are only in
the JSON. See [JSON outputs](/reference/json/#results).

### Tidy table

A table with one observation per row and one variable per column, one table per kind of
observation. DiMA's TSV tables are tidy: positions, variants, metadata (one row per variant, field
and value) and conserved regions. See [TSV tables](/reference/tsv/).

### NA

The marker of an undefined or not requested value in the TSV tables, where the JSON has `null`. It
appears only in the columns of the positions table that allow it; a k-mer or header value that
reads `NA` elsewhere is text. See [TSV tables](/reference/tsv/#format).
