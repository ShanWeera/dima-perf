---
title: Reading alignments
description: How DiMA reads an alignment — FASTA records, compression, line endings and white space, the alphabet decision ported from HMMER's Easel, symbol classes, and the cleaning of header metadata.
---

DiMA's results depend only on the alignment's residues and headers, never on how the file
happens to be formatted. A line-wrapped, lower-case, CRLF or compressed copy of an alignment gives
the same positions, variants, summaries and tables as the plain original; only the `run` record
of how the file was read (its name, size, checksum, compression and characters) differs. This page
explains the rules that make this so.

## FASTA records

The input is FASTA [@pearson1988]: each record starts with a `>` line, the **header**, followed
by sequence lines up to the next `>` or the end of the file.

- The header is the whole line after `>` (with a final carriage return removed).
- Sequence lines are joined; line breaks never enter a k-mer. Space, tab, carriage return,
  vertical tab and form feed inside sequence lines are ignored. White space at the end of a line
  is dropped silently; white space between residues is ignored too but counted and reported
  (`interior_whitespace`), because it often means the file was reformatted by an editor.
- A carriage return directly before a line feed is part of the line ending, so Windows (CRLF)
  files read like Unix (LF) files. One UTF-8 byte-order mark at the start is skipped, and so are
  lines before the first record that are blank or hold only white space (space, tab, carriage
  return, vertical tab, form feed), as MAFFT, Clustal Omega, EMBOSS and HMMER read them; the
  first other line must start with `>` in its first column.
- Every record must have the same number of **alignment columns** (characters other than white
  space) as the first one; an empty sequence is an error. DiMA needs an existing multiple
  sequence alignment, made for example with MAFFT or Clustal Omega
  [@katoh2013; @sievers2011].

Problems with the file's structure always stop the run, in every validation mode, with as much
of their location as applies — the record, line, alignment column and byte: no `>` before the
first sequence, a FASTQ file (`@` headers), a NUL byte, a carriage return inside a header
(classic Mac OS line endings), an empty sequence, unequal lengths, UTF-16 or UTF-32 text.
Records are never skipped. Records, lines and columns are counted from 1; the byte is an offset
from 0 in the decompressed text, so it can be found with `cmp`, `dd` or a hex editor after
decompressing the file.

## Compression and sources

The format is recognised from the first bytes, not from the file name: plain text, gzip
(including BGZF, the block-gzip format of SAMtools and HTSlib [@li2009; @bonfield2021]), bzip2
and Zstandard. Files made of several concatenated streams (joined with `cat`, or written by
`pbzip2`) are read to the end, and Zstandard frames written with `--long` windows up to 2 GiB are
accepted. xz, Unix `compress` (`.Z`), LZ4, legacy LZMA, zip and 7z files are refused with a
message that names the format and says how to convert it; an xz file, for example, can be
decompressed with `xz -d` or recompressed once with Zstandard, which keeps it as small
([large alignments](/how-to/large-alignments/#compress-the-input-with-zstandard)).

The input can be a file, standard input (`-i -`, or no `-i` with a pipe), a named pipe or a shell
process substitution such as `<(zcat a.fa.gz)`. Pipes and standard input are read in a single
pass, without seeking, so they work exactly like files.

While reading, DiMA computes the SHA-256 checksum of the stored bytes (compressed, if the file
is compressed) and records it with the file name and size in `run.input`, so a result can always
be matched to the file it came from.

## The alphabet

The alphabet — protein, DNA or RNA — decides which letters are residues. `--alphabet auto` (the
default) detects it with the rules of `esl_abc_GuessAlphabet()` from Easel, the sequence library
of HMMER [@eddy2011], ported rule by rule. They are applied to the letter counts (case
insensitive) of the first batch of records — at most 10,000 records or 64 MiB, and at least the
first 1,000 records (or all of them, if there are fewer); with $n$ letters in total:

1. $n \le 10$: undetermined (too little evidence);
2. $n > 2000$ and every letter is N: DNA;
3. any of E, F, I, J, L, O, P, Q, Z: protein;
4. at least 98% A, C, G, T or N, and all of A, C, G and T present: DNA;
5. at least 98% A, C, G, U or N, and all of A, C, G and U present: RNA;
6. at least 98% amino-acid letters (or N, T, X), more D, H, K, M, R, S, V, W, Y than A, C, G,
   and at least 15 different letters: protein;
7. otherwise undetermined.

Easel tuned these rules to make no false calls; when they do not decide, DiMA stops and asks for
`--alphabet protein`, `dna` or `rna` instead of guessing. (As Easel's code is written, rule 6
cannot fire, because without rule 3's letters at most 14 of its letters can occur; the port keeps
this.) A sequence set with substantial amounts of both T and U is undetermined too. When an
explicit `--alphabet` contradicts what the rules would detect, the run continues with the
explicit choice and reports `alphabet_contradiction`. The decision, its method (`detected` or
`user`) and the letters examined are recorded in `run.alphabet`.

## Symbols

Letters are folded to upper case first (soft-masked or lower-case alignments are common) and the
number folded is reported. T and U are one symbol, spelled as the resolved alphabet spells it.
Every other byte belongs to one class:

| Class | Protein | DNA / RNA | Effect |
|---|---|---|---|
| residue | `A C D E F G H I K L M N P Q R S T V W Y U O` | `A C G T` / `A C G U` | counted |
| gap | `- . _` | `- . _` | the k-mer is invalid |
| missing | `~ ?` | `~ ?` | the k-mer is invalid |
| ambiguity | `X B J Z` | `R Y K M S W B D H V N X I` | the k-mer is invalid |
| stop | `* $ ^` | `* $ ^` | the k-mer is invalid |
| frameshift | `#` | `#` | the k-mer is invalid |
| white space | space, tab, CR, VT, FF | same | ignored |
| unknown | anything else | anything else | depends on the validation mode |

The sources: the nucleotide ambiguity codes are the IUPAC-IUB recommendations
[@cornishbowden1985], with X and I (inosine, used in probes [@martin1985; @ohtsuka1985])
treated as ambiguous too; U (selenocysteine) and O (pyrrolysine) are genetically encoded amino
acids [@bock1991; @srinivasan2002]; `*` marks a stop codon in translated sequences and is a
non-residue in Easel, and `$` and `^` (stop), `#` (frameshift) and `?` (no consensus) are the
symbols of the Los Alamos HIV Sequence Database alignments [@lanlcodes; @lanlgenecutter], which
keep sequences with inactivating frameshifts [@kuiken2003]; `.` and `_` as gaps and `~` as
missing data follow Easel [@eddy2011]; and spaces, tabs and carriage returns inside sequence
lines are ignored as Easel's FASTA reader ignores them [@eddy2011], with vertical tabs and form
feeds (the rest of the POSIX `space` class) ignored too. The complete table, byte by
byte, is in [Symbol tables](/reference/symbols/).

Treating stops and frameshifts as non-residues matters: on the HIV-1 *pol* protein alignment,
which contains `*` and `#`, it moves 115 positions from LS to NS, and the mean
entropy over eligible positions becomes 1.9072 bits (paper: about 1.91), against 1.9125 when they
are counted as residues, as DiMA v5.0.9 did (both computed with the independent reference
implementation in `scripts/reference/`, the second with v5.0.9's symbol rules substituted).

### Unknown characters and validation modes

Characters of no class above (digits, other punctuation, letters outside the alphabet) are
handled by `--validation`:

- `strict` (default): the run stops at the first one, with its record, line and column;
- `permissive`: the k-mers that contain them are excluded, and one aggregated warning
  (`unknown_characters_excluded`) gives their count and first occurrences;
- `report`: as permissive, and the table of every character found is printed.

Every character found is always counted by class in `run.validation.characters`. See
[Validate input characters](/how-to/validation/).

## Header metadata

With `--header-format`, each header is split on `|` into named fields, as in the paper
[@tharanga2025]. The whole header line is split *before* any cleaning, so cleaning can never
create or remove a delimiter. Each value is then cleaned in a fixed order:

1. invisible characters are removed: Unicode format characters (General Category Cf, such as
   zero-width spaces and bidirectional marks [@uax44]) and control characters except tab;
2. Unicode white space is trimmed from both ends;
3. the value is normalised to Unicode NFC [@uax15; @charmodnorm], so that the same text typed
   with precomposed or combining characters counts as one value (compatibility forms are not
   folded: that would change visible text);
4. an empty value becomes the fill value (`--header-fillna`, default `Unknown`).

Nothing else is changed: case, punctuation and the spaces inside a value are kept. Every change is counted in
`run.validation.header_cleaning`, together with the Unicode version used. Headers that are not
valid UTF-8 stop a strict run with the record and byte (convert the file, for example with
`iconv`); permissive runs replace the invalid bytes with U+FFFD and count them. What happens to
headers with the wrong number of fields, and the warnings about identifier-like fields,
missing-value tokens and near-duplicate spellings, is described in
[Stratify by header metadata](/how-to/metadata/).

## The query name

The results carry the name of the analysed protein or gene, `query_name`. Unless
`--query-name` sets it, it is the input file name without its compression and FASTA extensions,
stripped repeatedly and case-insensitively, as MultiQC derives sample names [@ewels2016]:
`spike.fasta.gz` gives `spike`. The recognised extensions are `.gz .bgz .bz2 .xz .zst` and
`.fasta .fas .fa .fna .ffn .faa .frn .afa .aln .mfa`. Standard input gives `Unknown Protein`,
`Unknown DNA` or `Unknown RNA`. Where the name came from is recorded in `run.parameters`.

## References

[^ref]
