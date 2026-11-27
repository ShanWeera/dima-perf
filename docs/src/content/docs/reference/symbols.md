---
title: Symbol tables
description: The class of every character in protein, DNA and RNA alignments — residues, gaps, missing data, ambiguity codes, stops, frameshifts, white space and unknown characters — with their sources.
---

Every byte of a sequence line belongs to exactly one class, which depends on the alphabet. The
classes are the values of `class` in `run.validation.characters` of the
[results](/reference/json/#run). Letters are case-insensitive: lower-case letters are folded to
upper case first (and counted).

| Class | Effect at a position |
|---|---|
| `residue` | part of the k-mer |
| `gap`, `missing`, `ambiguity`, `stop`, `frameshift` | the k-mer is invalid: that sequence does not count towards the support |
| `whitespace` | ignored (the sequence continues) |
| `unknown` | strict mode: the run stops with record, line and column; permissive and report modes: the k-mer is invalid and the character is counted and reported |

## Protein

| Class | Characters | Source |
|---|---|---|
| residue | `A C D E F G H I K L M N P Q R S T V W Y` | the 20 standard amino acids |
| residue | `U` (selenocysteine), `O` (pyrrolysine) | genetically encoded amino acids [@bock1991; @srinivasan2002] |
| ambiguity | `X` (any), `B` (D or N), `Z` (E or Q), `J` (I or L) | IUPAC-IUB one-letter codes |
| gap | `-` `.` `_` | alignment gaps, as Easel, the sequence library of HMMER [@eddy2011], reads them |
| missing | `~` `?` | missing data: `~` as Easel, the sequence library of HMMER [@eddy2011], reads it; `?` is "no consensus" in LANL alignments [@lanlcodes] |
| stop | `*` `$` `^` | `*` is the usual stop symbol of translations; `$` and `^` are stops in LANL alignments [@lanlcodes; @lanlgenecutter] |
| frameshift | `#` | LANL alignments [@lanlcodes; @lanlgenecutter] |
| whitespace | space, tab (`\t`), carriage return (`\r`), vertical tab, form feed | line formatting; the FASTA reader of Easel, the sequence library of HMMER [@eddy2011], ignores the first three in sequence lines |
| unknown | every other byte: digits, other punctuation, the letters not listed above, non-ASCII bytes | |

## DNA

| Class | Characters | Source |
|---|---|---|
| residue | `A C G T` | |
| residue | `U`, counted as `T` | T and U are one symbol; the number folded is reported |
| ambiguity | `R Y K M S W B D H V N` | IUPAC-IUB nucleotide codes [@cornishbowden1985] |
| ambiguity | `X`, `I` (inosine) | "any" and inosine, which pairs with several bases [@martin1985; @ohtsuka1985] |
| gap, missing, stop, frameshift, whitespace | as for proteins | |
| unknown | every other byte | |

## RNA

As DNA, with `U` as the residue and `T` counted as `U`.

## Bytes that never reach the table

- The line feed ends a line; a carriage return directly before it is part of the line ending.
- A NUL byte (0x00) anywhere stops the run in every mode: the file is binary or corrupt.
- Header lines (starting with `>`) are not sequence data; see
  [header metadata](/explanation/input/#header-metadata).

## Why these choices

Folding case, treating T and U as one symbol, excluding stops and frameshifts, and stopping at
unknown characters by default are deliberate; the reasons, and the effect on the paper's HIV-1
*pol* protein dataset, are in [Reading alignments](/explanation/input/#symbols). DiMA v5.0.9 used
different rules; see [Differences from DiMA v5.0.9](/explanation/differences/#input).

## References

[^ref]
