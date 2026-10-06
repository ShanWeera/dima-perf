---
title: Validate input characters
description: Decide what happens to characters outside the alphabet with --validation strict, permissive or report, and inspect what an alignment contains.
---

Before counting k-mers, DiMA classifies every character of every sequence (see the
[symbol tables](/reference/symbols/)). Residues are counted; gaps, missing data, ambiguity codes,
stops and frameshifts make the k-mers that contain them invalid; white space is ignored. A
character of none of these classes — a digit, stray punctuation, a letter that is not a residue
of the alphabet — is **unknown**, and `--validation` decides what happens to it.

## The three modes

| Mode | Unknown characters | Headers with the wrong number of fields, invalid UTF-8 in headers |
|---|---|---|
| `strict` (default) | the run stops at the first one, with record, line and column | the run stops (all headers are read first, and the first 20 offenders are listed) |
| `permissive` | the k-mers that contain them are excluded; one warning gives the count and the first occurrences | accepted: fill value, or U+FFFD replacement, with one warning |
| `report` | as permissive, and the table of every character found is printed | as permissive |

Problems with the file's structure stop the run in every mode: data before the first header,
FASTQ, NUL bytes, carriage returns inside headers, sequences of unequal length, UTF-16 text,
corrupt compressed data. DiMA never skips a record silently.

## Strict: find the problem

```console
$ dima analyze -i bad.fasta -a protein -o bad.json
error: bad.fasta: record 1 (line 2, column 10, byte 12): character '1' (0x31) is not valid in a protein alignment
  hint: check --alphabet, or use --validation permissive to exclude the k-mers that contain such characters (they are counted and reported)
```

The column is the alignment column (white space not counted), so you can find the character in
an alignment viewer. Typical causes: a wrong `--alphabet`, numbers or annotations pasted into a
sequence, or an alignment exported with unusual symbols.

## Report: see what the file contains

```sh
dima analyze -i alignment.fasta --validation report -o alignment.json
```

`report` analyses like `permissive` and prints, on standard error, every character found in the
sequence lines with its byte value, class and count. The same table is always stored in the
results, whatever the mode:

```sh
jq -r '.run.validation.characters[] | "\(.char // "·")\t\(.class)\t\(.count)"' alignment.json
```

For the MERS-CoV spike sample, it shows the 20 residues, the gap `-` and the ambiguity codes `X`
(587 times), `B` (once) and `Z` (twice).

## Permissive: analyse anyway

When the unknown characters are rare and you have checked what they are, `permissive` excludes
the k-mers that contain them and continues. The warning `unknown_characters_excluded` reports how
many there were and where the first ones are; `run.validation.unknown_total` gives the total.
Excluded k-mers lower the support of their positions, exactly like gaps.

## Other things DiMA reports while reading

- **Lower case** is read as upper case (`lowercase_folded`, shown with `-v`).
- **T and U**: U in a DNA alignment is read as T, and T in an RNA alignment as U
  (`t_u_folded`, shown with `-v`).
- **White space between residues** is ignored but reported (`interior_whitespace`), because it
  usually means the file was reformatted.

All of these are in `run.validation` and `run.diagnostics`; see [Diagnostics](/reference/diagnostics/).
