---
title: Differences from DiMA v5.0.9
description: Every documented difference between this implementation and DiMA v5.0.9, the published implementation of the paper, with the reason for each.
---

DiMA v5.0.9 (`dima-cli`, tag `v5.0.9` of the original repository, MIT licence) is the
implementation published with the paper [@tharanga2025]. The paper's method is the specification
of this implementation; it departs from v5.0.9 only where v5.0.9 departs from the paper, where a
result was undefined or unstable, or where an engineering change leaves results unchanged.
The optional methods that are in neither v5.0.9 nor the paper, the alternative estimators and
similarity-weighted entropy, change nothing unless they are requested.
Results of v5.0.9 can be compared with DiMA's at a relative tolerance of about $10^{-5}$ for
incidences (v5.0.9 stores them in single precision) and statistically for entropies (v5.0.9's
correction is random).

## Method

| Topic | DiMA v5.0.9 | This implementation | Why |
|---|---|---|---|
| Entropy correction | one random subsample per grid point, unseeded, so every run differs | the exact expectation of the same procedure | the intercept's mean without its resampling noise, reproducible [@rao1945; @blackwell1947]; see [the exact expectation](/explanation/entropy/#the-exact-expectation) |
| Negative intercept | the observed entropy reported for each random run whose intercept is negative | the expected intercept is never negative, so the rule never applies; any reported entropy below 0 would be an internal error | the paper attributes negative values to too few resamples; applied per run, the rule raises the average at near-conserved positions; see [the negative-intercept rule](/explanation/entropy/#the-negative-intercept-rule) |
| Subsample sizes | rounded down, $\lfloor RN/100 \rfloor$ | rounded up, $\lceil RN/100 \rceil$ | Fig. 2 ("Upperbound") |
| Grid start | $\lceil 100T/N \rceil$ in floating point ($T = 55$, $N = 100$ gives 56) | exact integer arithmetic | the paper's formula; identical for $T = 30$ and $T = 100$ |
| Positions without support | entropy 0, as the paper's "pseudo entropy" of 0 (Fig. 2; Methods) | no entropy (`null`, `NA`), still tagged NS | a deliberate departure from the paper as well: an unobserved value is missing, not zero; see [positions without support](/explanation/entropy/#positions-without-support) |
| Ratios $0/0$ | 0 | `null` (`NA`) | a fully conserved position must not look like a position without data; see [undefined ratios](/explanation/motifs/#undefined-ratios) |
| Incidence precision | single precision (`f32`) | double precision | |
| Summary statistics | mean over all positions with NS counted as 0; highest entropy at the last maximum, 0-based; no lowest; a crash without positions | mean, lowest and highest over eligible positions, first position (1-based) and tie count, `null` when empty; v5.0.9's mean kept as `all_positions_summary` | the paper reports statistics of positions with support above $T$; see [summaries](/explanation/motifs/#run-summaries) |
| Default support threshold | 30 | 100 | the paper's text: "default is 100 sequences, arbitrarily defined", repeated in its HIV-1 application (its Fig. 2 shows 30, v5.0.9's default) |
| Default k | 9 for every alphabet | 9 for proteins, 27 for nucleotides | 27 nucleotides encode a 9-mer |
| Estimators | the correction only | the correction (default), Chao–Wang–Jost, Miller–Madow, raw, and the sample coverage | [@chao2013; @miller1955; @chao2012] |
| Similarity-weighted entropy | not computed | on request (`--similarity-weighted-entropy`), similarity-weighted entropy and the observed and similarity-weighted entropies divided by $\log_2$ of the number of distinct k-mers; off by default, and every other value is the same with or without it | an addition from Wu, not part of the paper or of v5.0.9 [@wu2025]; see [similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) |

## Conserved sequences

| Topic | DiMA v5.0.9 | This implementation |
|---|---|---|
| Threshold | any value, compared in single precision | above 50% and at most 100%, compared exactly |
| Candidates | every Index variant, tied ones included | one Index per position (guaranteed above 50%) |
| Joining | by text: when the end of the accumulated sequence overlaps the start of the next k-mer by at least one character, its last residue is appended, whatever the positions | by position: the next k-mer must overlap or abut the region and agree with it on every shared column; a disagreement starts a new region |
| Low support | not reported | regions with positions of support $\le T$ are flagged, and each reports an exact 95% Clopper–Pearson lower bound [@clopper1934]; `--hcs-eligible-only` excludes such positions |
| No region | `[""]` | an empty list |

On the HIV-1 *pol* DNA dataset at 70%, both give the same two regions. At 100%, v5.0.9 reports
two conserved strings on the HIV-1 *pol* protein alignment and eight on the MERS-CoV spike gene,
which include positions supported by only 2 or 3 sequences; DiMA reports the corresponding
regions flagged, with lower bounds of 29.2% and 15.8%. See
[Historically conserved sequences](/explanation/conserved-sequences/).

## Input

| Topic | DiMA v5.0.9 | This implementation | Why |
|---|---|---|---|
| Line-wrapped FASTA | lines joined after trimming their ends | all white space inside sequence lines ignored | FASTA files are commonly wrapped; formatting must not change results |
| Lines before the first record | any line, even a blank one, stops the program | blank and white-space-only lines skipped; anything else is an error with its line and column | they carry no data; MAFFT, Clustal Omega and EMBOSS, which these pages recommend, skip them too |
| Case | case-sensitive (`a` and `A` differ) | folded to upper case, count reported | soft-masked and lower-case alignments are common |
| Alphabet | not detected | detected with Easel's rules, or set with `--alphabet` [@eddy2011] | no false calls; asks when undetermined |
| T and U | different symbols | one symbol, spelled per alphabet | a mixed T/U file is a formatting artefact |
| Protein U and O | excluded | residues | selenocysteine and pyrrolysine are encoded amino acids [@bock1991; @srinivasan2002] |
| Nucleotide X and I | residues | ambiguity codes, excluded | not one of A, C, G, T/U [@cornishbowden1985] |
| `.` `*` `#` `$` and spaces between residues | residues | gap, stop, frameshift, stop: excluded; spaces ignored | LANL and Easel symbol conventions [@lanlcodes; @eddy2011] |
| Other characters (digits, punctuation) | residues | an error (strict) or excluded with a warning | a digit or stray symbol is never a residue |

## Headers and metadata

| Topic | DiMA v5.0.9 | This implementation |
|---|---|---|
| Field values | split on `\|`, trimmed, empty ones filled | split on `\|`, invisible characters removed, Unicode white space trimmed, NFC-normalised, empty ones filled [@uax15] |
| Wrong number of fields | the program stops with an assertion failure | strict: an error listing the first 20 headers; permissive: their fields get the fill value, with a warning |
| Identifier-like and missing-like values | not detected | warnings with counts, examples and a remedy |
| Records counted for a variant's metadata | the header at the variant's index among *all* records, although the index counts only the records with a valid k-mer at that position: when an earlier record has none there, the values of other records are counted | the headers of the records that carry the variant |
| Query name | `Unknown Query` | the input file name without its extensions |

The paper's Fig. 1C was made with v5.0.9 and shows this shift: its host chart for the Index
variant of MERS-CoV spike position 25 gives *Homo sapiens* 81 of 129 sequences and "others" 3,
where the records that carry the variant give 80 and 4 (the other hosts agree).

## Outputs

The JSON keys of v5.0.9 are kept (`query_name`, `support_threshold`, `low_support_count`,
`highest_entropy`, `results`, `position`, `low_support`, `entropy`, `support`,
`distinct_variants_count`, `distinct_variants_incidence`, `total_variants_incidence`,
`diversity_motifs`, `sequence`, `count`, `incidence`, `motif_short`, `motif_long`, `metadata`).
New keys add what v5.0.9 left implicit: positions are 1-based, the method and fallback of each
entropy, the observed entropy, the sample coverage, the summaries described above, and the full
provenance of the run (`run`: parameters with their origin, input checksum, alphabet decision,
validation, diagnostics and references). With `--similarity-weighted-entropy`, each position also
carries its similarity-weighted and normalised entropies and the results a summary of them;
without it, these keys are `null`. See [JSON outputs](/reference/json/).

## References

[^ref]
