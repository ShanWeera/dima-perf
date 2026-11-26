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

## Results files (exit 4)

**`written with results schema …`** (or **`written with conserved-sequences schema …`**; from
`dima view`, `dima hcs` or `dima cite --from`) — the major number of the file's schema version
(before the first dot) is not `1`: it was written by a DiMA version with another major schema
version, or its version is not a plain number, such as `01.1.0`. Re-run the analysis, or use the version that wrote the file.

**`position N: … is given, but …`**, **`… is missing`** or another message with the hint `the
file was edited or damaged` (exit 4, from `dima view`, `dima hcs` or `dima cite --from`) — the
results contradict themselves, for example a similarity-weighted value at a position without
support, no such value where the run computed them (`run.parameters.similarity_weighted_entropy`
says whether it did), or `position N: similarity_weighted_entropy is …, above the observed
entropy …, which it cannot exceed`. DiMA never writes such a file; it was changed after `dima analyze` wrote
it. Run the analysis again; if you edit results by hand, keep the rules of the
[results reference](/reference/json/).

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

**`flag_has_no_effect`: `--similarity-weighted-entropy has no effect without results (JSON,
JSONL) or the positions table`** — the values of similarity-weighted entropy are written only
to the results and the positions table, and none of your outputs is one of them (for example,
only `-o variants.tsv --table variants`, or only `--hcs-output`). DiMA then does not compute
them and writes the other outputs as usual. Add `-o PATH.json`, or the positions table with
`-o PATH.tsv --table positions` or `--tsv-dir DIR`. The same code marks other options that do
nothing with the chosen outputs; see
[Diagnostics](/reference/diagnostics/#warnings-of-the-command-line).

**`threads_refused`** — the system started fewer worker threads than requested, so the run was
slower than it could be; the results are unaffected. Ask for fewer threads (`-t`) or raise the
limit on processes and threads (`ulimit -u`, a container's pids limit).

All diagnostics are explained in [Diagnostics](/reference/diagnostics/).

## Results written by another DiMA version

`dima view`, `dima hcs`, `dima cite --from` and the desktop app read results by their schema
version (`schema_version`; see [Comparing versions](/how-to/reproducibility/#comparing-versions)).
Results of another major schema version are refused (`written with results schema …`, above).

**`note: results file written with schema 1.0.0; values that schema did not record are written
as null (NA in the positions table), and options it did not have as off`** (from `dima view`) —
the file was written by dima 1.0.0, before similarity-weighted entropy existed. `dima view`
converts it as a run without that option: the new keys are `null` (`NA` in the positions table)
and `run.parameters.similarity_weighted_entropy` is `false`. The note appears only when the output
holds values per position (results JSON or JSONL, or the positions table). Nothing needs doing; to
get the values, run the analysis again with `--similarity-weighted-entropy`.

**`newer_schema`: `the results were written with schema …; this version writes … and keeps only
the fields it knows`** (from `dima view`) — a newer DiMA wrote the file, or its 1.x version is
one this DiMA does not recognise (such as `1.1` or `1.1.0-dev`), so it may hold keys that this
version does not know, and the converted output leaves them out. To keep every key, convert
the file with the DiMA version that wrote it (its `run.tool.version`) or a newer one. If the file
also holds a value this version does not know, such as the name of a new estimator, reading stops
instead (exit 4), and the fix is the same. The message is `written by a newer DiMA … with values
this version … does not know` when the version is a newer one, such as `1.2.0`, and `not a valid
DiMA results file: unknown variant …` when it is not recognised.

**`Warning: The results were written with schema …; this version writes … and keeps only the
fields it knows. Use the DiMA version that wrote the file to keep every field.`** (in the desktop
app, when it opens a results file) — the same case as `newer_schema` above: a newer DiMA wrote
the file, or its version is one this DiMA does not recognise (then shown in quotes, such as
`"1.1.0-dev"`). The app shows the results as usual, but an export of them (results JSON, JSON
Lines, the TSV folder) leaves out what only the newer version records. To keep everything, open
the file in the DiMA version that wrote it (its `run.tool.version`) or a newer one. Press the
warning's × to dismiss it.

## Other questions

**The results differ from the paper or from DiMA v5.0.9.** See
[the FAQ](/getting-started/faq/#why-do-my-results-differ-from-the-paper-or-from-dima-cli-5x).

**The similarity-weighted values are `NA` (`null`).** There are three reasons. The run did not
ask for them: they are computed only with `--similarity-weighted-entropy` (in the desktop app,
**Compute similarity-weighted entropy (Wu 2025)** under **Advanced**), and
`run.parameters.similarity_weighted_entropy` is then `false`; results written by dima 1.0.0,
which predate the option, never have them. Or the position has no support (NS), and all three
values are undefined. Or, for the two normalised
values only, the position has a single distinct k-mer (a fully conserved position): its
similarity-weighted entropy is 0, and the normalised values are undefined because the largest
entropy possible there is 0. See
[similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in).

**The log is full of escape codes or progress lines.** The progress line appears only on
terminals that can redraw a line (not with `TERM=dumb`), and uses no escape codes; if your
terminal or screen reader still has trouble with it, use `--progress plain` (appended lines)
or `--progress off`; `DIMA_PROGRESS` sets the default. Colours follow `--color` and
`NO_COLOR`; see [Environment variables](/reference/environment/#colour).

**`error: internal error`** (exit 1) — a bug. Please report it in
[GitHub Issues](https://github.com/ShanWeera/dima-perf/issues/new/choose) with the full message (it
includes the version and platform), the command, and if possible the input; `RUST_BACKTRACE=1`
adds a backtrace.

**The desktop app does not open** on macOS or Windows — the packages are not code-signed yet; see
[first launch](/getting-started/install/#desktop-app).

**Desktop app: "Nothing was saved: the chart was switched to similarity-weighted entropy while
the dialog was open. Export again to save the chart now shown."** (or "…switched to entropy…")
— the chart showed another metric when you chose the figure export than when you confirmed the
file name, so the file would have held the other metric's figure under a name chosen for this
one. Choose the export again; the save dialog then proposes the name of the chart now shown. See
[Export](/how-to/desktop-app/#export).

**Desktop app: "No positions are shown, so there is no chart to export (check the filters)"** —
the filter bar hides every position, so the chart is empty. Widen the filters (or press
**Reset**) and export again. When the filters were changed while the save dialog was open, the
message starts with "Nothing was saved:" and nothing is written.

**Desktop app: "Value range reset to the full range of similarity-weighted entropy"** (or "…of
entropy") — you switched the chart's metric after narrowing the value range in the filter bar.
Each metric has its own scale, so the range starts over at the new metric's full range; narrow
it again if you need to. See
[Plot similarity-weighted entropy](/how-to/desktop-app/#explore-the-results).

**Desktop app: "Exported figures are now at least 720 × 320 px so the title, legend and axes
fit; yours was raised to W × H px"** (or "Exported figures are between 720 × 320 and 20000 ×
20000 px; yours was changed to W × H px") — the figure size saved in the settings, by an earlier
version or by hand, was outside the sizes exported figures can have. The app moved it into that
range once and saved it; nothing else changed. Nothing needs doing; to choose another size, set it
in **Preferences**. See [Open an alignment](/how-to/desktop-app/#open-an-alignment).

For anything this page does not answer, see the [FAQ](/getting-started/faq/) or ask in
[GitHub Discussions](https://github.com/ShanWeera/dima-perf/discussions).
