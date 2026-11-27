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

Headers are checked only with `--header-format`: without it, DiMA does not read them as
metadata, so their field count and their UTF-8 do not matter, and an invalid byte in a header
never stops the run.

`permissive` turns two kinds of problem into warnings: unknown characters in the sequences,
whose k-mers are then excluded, and headers with the wrong number of fields, whose
[stratified fields](/reference/glossary/#stratified-field) then get the
[fill value](/reference/glossary/#fill-value). Read its warnings: each one tells what was
changed.

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

## A wrong `--alphabet`

With `-a dna`, a protein alignment stops at its first amino acid that is not a nucleotide letter:

```console
$ dima analyze -i mers_spike_aa.fasta -a dna -o w.json
error: mers_spike_aa.fasta: record 1 (line 2, column 6, byte 43): character 'F' (0x46) is not valid in a dna alignment
  hint: check --alphabet first: the residues of the first records look like protein, not dna; or use --validation permissive to exclude the k-mers that contain such characters (they are counted and reported)
```

Check `-a` first: here the residues are amino acids, so the fix is `-a protein`, or no `-a` at
all, which detects the alphabet. `--validation permissive` would not fix it: it would exclude
every k-mer with an amino acid that is not a nucleotide letter and analyse what is left as DNA,
with a warning that the residues look like another alphabet (`alphabet_contradiction`). The
same warning comes in any mode when every character is valid in the alphabet you gave but the
composition looks like another, as with `-a protein` on a DNA alignment.

## Report: see what the file contains

```sh
dima analyze -i alignment.fasta --validation report -o alignment.json
```

`report` analyses like `permissive` and prints, on standard error before the summary line, every
character found in the sequence lines with its byte value, class and count. For the MERS-CoV
spike sample:

```text
characters in sequence lines:
  char   byte  class         count
  -      0x2D  gap           22077
  A      0x41  residue       15330
  B      0x42  ambiguity     1
  C      0x43  residue       7294
  D      0x44  residue       11499
  E      0x45  residue       8000
  F      0x46  residue       12443
  G      0x47  residue       15993
  H      0x48  residue       3463
  I      0x49  residue       12736
  K      0x4B  residue       8889
  L      0x4C  residue       20959
  M      0x4D  residue       3671
  N      0x4E  residue       13405
  P      0x50  residue       10818
  Q      0x51  residue       12346
  R      0x52  residue       7800
  S      0x53  residue       23452
  T      0x54  residue       16011
  V      0x56  residue       16651
  W      0x57  residue       1737
  X      0x58  ambiguity     587
  Y      0x59  residue       13259
  Z      0x5A  ambiguity     2
analysed 191 sequences, 1345 positions (1345 eligible) in 0.03 s; entropy mean 0.1261, min 0.0000, max 1.5878 bits
```

The 20 residues, the gap `-` and the ambiguity codes `X` (587 times), `B` (once) and `Z` (twice).
Characters are counted byte by byte, in byte order. The `char` column is left blank for a byte
that has no printable form of its own — white space such as a space (`0x20`) or a tab (`0x09`),
and each byte of a character outside ASCII, such as the two bytes of `é` (`0xC3`, `0xA9`) — so
the byte identifies it. `-q` hides the table, like every message but errors.

The same table is always stored in the results, whatever the mode, so you can read it later:

```sh
jq -r '.run.validation.characters[] | "\(.char // "·")\t\(.class)\t\(.count)"' alignment.json
```

where `·` stands for a byte without a printable form (`char` is `null` in the results).

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
