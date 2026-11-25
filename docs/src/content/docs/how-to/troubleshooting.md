---
title: Troubleshooting
description: The messages you are most likely to meet when running DiMA, what they mean and how to fix them.
---

Every DiMA error names the problem, where it occurred and how to fix it (the `hint:` line). This
page collects the common ones with more context. The exit status tells the class of problem:
2 for the command line, 3 for files, 4 for the data; see [Exit codes](/reference/exit-codes/).

## The command line (exit 2)

**`cannot tell the output format from '…'`** — the output name has no `.json`, `.jsonl`,
`.ndjson`, `.tsv` or `.tab` extension (optionally followed by `.gz` or `.zst`). Rename it or add
`-O json|jsonl|tsv`; for example `-o /dev/null -O json`.

**`TSV output holds one table; which one?`** — `-o x.tsv` writes one of four tables. Add
`--table positions|variants|metadata|hcs`, or write all four with `--tsv-dir DIR`.

**`-O … does not match the extension of '…'`** — `-O` and the file name disagree; DiMA does not
guess which one you meant. Drop `-O` or fix the extension.

**`'…' would be written twice`** — two outputs are the same file. Give each its own name.

**`output '…' is the input file`** — an output would replace the input, which DiMA never
overwrites. Choose another output path.

**`… may be the same file as …`** — two names differ only in letter case, which macOS and
Windows treat as one file; DiMA refuses them on every system so that a command never loses an
output there. Rename one.

**`k-mer length … is longer than the alignment`** — choose a `-k` between 1 and the number of
alignment columns.

**`HCS threshold … is outside (50, 100]`** — `--hcs-min-incidence` must be above 50 and at most
100; see [why](/explanation/conserved-sequences/#which-positions-qualify).

**`no input alignment`** — `dima analyze` was run without `-i` and without piped input. Give the
file with `-i`, or pipe it in.

**`RAYON_NUM_THREADS is not a whole number of at least 1`** — fix or unset the variable, or use
`--threads`.

## Files and system resources (exit 3)

**`cannot open '…'`** (an input) or **`cannot create '…'`** (an output) with `No such file or
directory` or `Permission denied` — check the path and the permissions. For outputs, the
directory must exist (except `--tsv-dir`, which is created) and be writable, because each output
is first written to a temporary file next to it. Outputs are checked before the analysis starts.

**`… is a directory`** (exit 2) — `-i` or the redirected standard input names a folder; give
the alignment file.

**`cannot start …`** (the reader thread, the checksum thread, the interrupt handler or a worker
thread) — the system refused a new thread (`Resource temporarily unavailable`): a limit on
processes and threads (`ulimit -u`, a container's pids limit; on macOS, the limit of
threads per process) or a lack of memory. Use fewer threads (`-t`), stop other programs, or raise
the limit.

## The alignment (exit 4)

**`only N residue letters in the first … records`**, **`… contain both T (…) and U (…)`** or
**`the residue composition … does not identify the alphabet`** — automatic detection could not
decide between protein, DNA and RNA. Say which with `-a protein`, `-a dna` or `-a rna`.

**`the sequence has … alignment columns, but the first record has …`** — the sequences are not
aligned, or a record is truncated. DiMA needs a multiple sequence alignment: align the sequences
first (MAFFT, Clustal Omega and similar tools), and check the record and line given.

**`character '…' is not valid in a … alignment`** — a character that is not a residue, gap or
known symbol of the alphabet (see [Symbol tables](/reference/symbols/)). Check `-a`; if the
characters are expected and rare, `--validation permissive` excludes the k-mers that contain them
and reports them, and `--validation report` lists every character in the file. See
[Validate input characters](/how-to/validation/).

**`this looks like FASTQ ('@' header)`** — DiMA analyses aligned FASTA, not sequencing reads.

**`expected a '>' header line, found …`** — the file is not FASTA, or something precedes the
first record. Convert other alignment formats first (Stockholm or Clustal, for example with
Easel's `esl-reformat afa in.sto > out.fasta`), and extract archives (`.tar`). `found 0x00`
means the file is binary, or UTF-16 or UTF-32 text without a byte-order mark; `found 0xC2`
usually means a no-break space (U+00A0), which is not white space in FASTA: delete it or
replace it with a space.

**`white space before the first header's '>'`** — the first header line is indented. A header
line starts with `>` in its first column; remove the spaces or tabs before it.

**`carriage return before the first header line`** — the file uses classic Mac OS line endings
(CR only) or has a stray carriage return; convert carriage returns to line feeds, for example
`tr '\r' '\n' < in.fasta > out.fasta`.

**`the input contains no FASTA records`** — the file is empty or holds only blank or
white-space lines.

**`the sequence is empty`** — a header line is not followed by a sequence (often a stray `>`
line, or two headers in a row). Remove the record or add its sequence.

**`NUL byte in a header line`** / **`NUL byte in a sequence`** — the file is binary, damaged, or
UTF-16/UTF-32 text; FASTA is plain text.

**`the input is … (xz, .Z, LZ4, legacy LZMA, a zip or 7z archive), which DiMA cannot read`** —
decompress it, or recompress it with gzip or zstd; for xz, `xz -dk NAME.xz`, or
`xz -dc NAME.xz | zstd -19 -o NAME.zst` to keep it compressed.

**`corrupt …` / `truncated … data`** — the compressed file is damaged or incomplete; test it
with `gzip -t`, `bzip2 -t` or `zstd -t`, and download or compress it again. The same applies to
results files read by `dima view`, `dima hcs` and `dima cite --from`.

**`N bytes that are not gzip data follow the end of the compressed data`** (or bzip2) — the
file has something appended after its compressed data. `gzip -d` and `bzip2 -d` ignore it with
a warning (zero bytes, padding, are accepted silently). Recompress the alignment without it,
for example `gzip -dc NAME.gz | gzip > fixed.gz`.

**`the text is UTF-16 encoded`** / **`UTF-32 encoded`** — convert it, for example
`iconv -f UTF-16 -t UTF-8 in.fasta > out.fasta` (or `-f UTF-32`).

**`carriage return inside a header line`** — in the first line, the file uses classic Mac OS
line endings (CR only): convert them with `tr '\r' '\n' < in.fasta > out.fasta`. Elsewhere, a
header holds a stray carriage return: remove it, for example with `tr -d '\r'`.

**`… headers do not have the … fields of the header format`** — with `--header-format`, every
header must have exactly the named number of `|`-separated fields. Fix the headers or the format,
or accept the mismatches with `--validation permissive` (they get the fill value). See
[Stratify by header metadata](/how-to/metadata/#headers-with-the-wrong-number-of-fields).

**`header is not valid UTF-8`** — convert the file to UTF-8 (`iconv -f LATIN1 -t UTF-8`), or use
`--validation permissive` to replace invalid bytes.

**`written with results schema …`** (from `dima view`, `dima hcs` or `dima cite --from`) — the
results were written by a DiMA version with another major schema version. Re-run the analysis, or
use the version that wrote them.

## Warnings worth acting on

**`threshold_exceeds_sequence_count`** or **`no_eligible_positions`** — no position has support
above the support threshold, so the summaries are empty (and, with the default estimator, every
entropy is uncorrected). Lower
`-s`, use a shorter `-k`, or check why so many k-mers are invalid (gap-rich columns, ambiguity
codes).

**`identifier_like_field`** — you are counting variants per accession or another unique
identifier; the output becomes very large. Use the `--metadata-fields` option the hint prints.

**`hcs_low_support_regions`** — some conserved regions rest on few sequences; see
[flagged regions](/how-to/conserved-sequences/#flagged-regions).

**`threads_refused`** — the system started fewer worker threads than requested, so the run was
slower than it could be; the results are unaffected. Ask for fewer threads (`-t`) or raise the
limit on processes and threads (`ulimit -u`, a container's pids limit).

All diagnostics are explained in [Diagnostics](/reference/diagnostics/).

## Other questions

**The results differ from the paper or from DiMA v5.0.9.** See
[the FAQ](/getting-started/faq/#why-do-my-results-differ-from-the-paper-or-from-dima-cli-5x).

**The log is full of escape codes or progress lines.** The progress line appears only on
terminals that can redraw a line (not with `TERM=dumb`), and uses no escape codes; if your
terminal or screen reader still has trouble with it, use `--progress plain` (appended lines)
or `--progress off`; `DIMA_PROGRESS` sets the default. Colours follow `--color` and
`NO_COLOR`; see [Environment variables](/reference/environment/#colour).

**`error: internal error`** (exit 1) — a bug. Please report it in
[GitHub Issues](https://github.com/ShanWeera/dima-perf/issues/new/choose) with the full message (it
includes the version and platform), the command, and if possible the input; `RUST_BACKTRACE=1`
adds a backtrace.

For anything this page does not answer, see the [FAQ](/getting-started/faq/) or ask in
[GitHub Discussions](https://github.com/ShanWeera/dima-perf/discussions).

**The desktop app does not open** on macOS or Windows — the packages are not code-signed yet; see
[first launch](/getting-started/install/#desktop-app).

**Desktop app: "Nothing was saved: the chart was switched to similarity-weighted entropy while
the dialog was open. Export again to save the chart now shown."** (or "…switched to entropy…")
— the chart showed another metric when you chose the figure export than when you confirmed the
file name, so the file would have held the other metric's figure under a name chosen for this
one. Choose the export again; the save dialog then proposes the name of the chart now shown. See
[Export](/how-to/desktop-app/#export).

**Desktop app: "Value range reset to the full range of similarity-weighted entropy"** (or "…of
entropy") — you switched the chart's metric after narrowing the value range in the filter bar.
Each metric has its own scale, so the range starts over at the new metric's full range; narrow
it again if you need to. See
[Plot similarity-weighted entropy](/how-to/desktop-app/#explore-the-results).
