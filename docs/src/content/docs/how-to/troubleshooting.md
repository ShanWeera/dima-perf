---
title: Troubleshooting
description: Every message DiMA prints — on the command line and in the desktop app — what it means and how to fix it.
---

Every DiMA error names the problem, where it occurred and how to fix it (the `hint:` line). This
page explains each message with its cause and its fix, grouped by the
[exit status](/reference/exit-codes/), which tells the class of problem: 2 for the command line,
3 for files and system resources, 4 for the data, 128 + N for an interruption. Messages are shown
as DiMA prints them; `…` stands for a name or a number from your command. Warnings and notes are
[diagnostics](/reference/glossary/#diagnostic); [Diagnostics](/reference/diagnostics/) gives
each one in full.

## The command line (exit 2)

Every usage error is found before the analysis starts, so it costs no analysis time; all but one
are found before the input is read. The exception is a k longer than the alignment, which DiMA
knows once it has read the first records.

### Options and their values

**``invalid value '0' for '--kmer-length <K>': "0" is not a whole number of at least 1``** (also
for `-s`, `-t`) — k, the support threshold and the thread count are whole numbers of at least 1.

**``invalid value '2' for '--min-coverage <X>': 2 is outside [0, 1]``** (or `"x" is not a
number`) — `--min-coverage` is a proportion between 0 and 1, such as `0.95`.

**``the argument '--quiet...' cannot be used with '--verbose...'``** — `-q` (errors only) and
`-v` (more) contradict each other; give one.

**``the argument '--licenses' cannot be used with 'analyze'``** (or another command) —
`dima --licenses` only prints the licences, so a command after it would never run. Run
`dima --licenses` on its own.

**`no input alignment`** — `dima analyze` was run without `-i` and without piped input. Give the
file with `-i`, or pipe it in (`dima analyze < alignment.fasta`). With `-i -` and nothing piped
in, DiMA waits for the alignment to be typed or pasted, ended by Ctrl-D.

**`k-mer length … is longer than the alignment (… columns)`** — k must be between 1 and the
number of alignment columns, which the hint gives. Without `-k`, the default is 9 for proteins
and 27 for DNA and RNA, so a short nucleotide alignment (6 columns, for example) needs `-k`
explicitly.

**`query name must not be empty`**, **`header fill value must not be empty`** — `--query-name
''` and `--header-fillna ''` are refused: give a name, or leave the option out for the default.

**`DIMA_PROGRESS is set, but not to a progress mode (auto, plain or off)`** — the variable holds
another value; values are lower case. Set it to `auto`, `plain` or `off`, or unset it
(`unset DIMA_PROGRESS`). An empty value counts as unset. See
[Environment variables](/reference/environment/#progress-the-order).

**`RAYON_NUM_THREADS is not a whole number of at least 1`** — fix or unset the variable, or use
`--threads`.

### Output options

**`cannot tell the output format from '…'`** — the name given to `-o` has no `.json`, `.jsonl`,
`.ndjson`, `.tsv` or `.tab` extension (optionally followed by `.gz` or `.zst`). Rename it or add
`-O json|jsonl|tsv`; for example `-o /dev/null -O json`.

**`TSV output holds one table; which one?`** — `-o x.tsv` writes one of four tables. Add
`--table positions|variants|metadata|hcs`, or write all four with `--tsv-dir DIR`.

**`--table applies to TSV output`** — `--table` was given with a JSON or JSON Lines output. Add
`-O tsv` or use a `.tsv` name, or drop `--table`.

**`--table applies to -o with TSV output`** — `--table` was given without `-o` while other
outputs are named (`--tsv-dir`, `--hcs-output`). Add `-o PATH.tsv`; `--tsv-dir` already writes
every table.

**`-O/--output-format applies to -o/--output`** — `-O` sets the format of `-o` only, and no
`-o` was given while other outputs are. Add `-o PATH` (or `-o -` for standard output).

**`-O … does not match the extension of '…' (…)`** — `-O` and the file name disagree; DiMA does
not guess which one you meant. Drop `-O` or fix the extension.

### Names DiMA refuses (since 1.2.0)

These names are refused before anything runs, whatever `-O` says; the rules are in
[Choose and read outputs](/how-to/outputs/#names-dima-refuses).

**`'…' asks for bzip2 compression, which DiMA does not write`** (or xz, LZMA, lzip, LZ4,
compress (.Z), Brotli, Snappy, BGZF; **`… asks for a ZIP archive`**, a 7-Zip archive, a tar
archive (`.tar`) or a compressed tar archive (`.tar.gz`, `.tar.zst`, `.tgz`, …)) — DiMA writes
one file per output, plain or compressed with gzip or Zstandard. Use `.gz` or `.zst`, as the hint
says; compress the file afterwards with another tool if you need its format.

**`'…' asks for Zstandard compression under the name .zstd`** (or gzip under `.gzip`) — DiMA
writes these, under `.zst` and `.gz`; rename as the hint says.

**`'…' asks for two compressions (.gz, then .zst); DiMA applies one`** — keep one compression
extension.

**`'…' names a folder, not a file`** — the name ends in `/` (or `\` on Windows), or its last part
is `.` or `..` (`out.json/.`). Give a file name, such as the one in the hint; with
`dima analyze` and `dima view`, `--tsv-dir` is the option that writes into a folder.

**`'…' is a folder, not a file`** — the output is an existing folder. Give a file name inside it,
as the hint suggests.

**`'…' is a file, not a folder`**, **`'…' is not a folder`** — `--tsv-dir` names an existing file,
or a device or FIFO such as `/dev/null`. Give it a folder name; it is created if needed.

**`'…' names JSON Lines, but conserved sequences are written as JSON or TSV`** — `--hcs-output`
or `dima hcs -o` was given a `.jsonl` or `.ndjson` name. Use `.json` or `.tsv`; the hint puts
`.hcs` in place of the JSON Lines extension (`x.jsonl` → `x.hcs.json`), so the name cannot be the
results' `x.json`.

**`the run report is JSON, but '…' names a TSV table`** (or `names JSON Lines`) — the run report
is always JSON. Name it `.json`, optionally followed by `.gz` or `.zst`; the hint suggests
`run.run-report.json` for `run.tsv`. No run report is written.

**`cannot create '…'`** with **`caused by: File name too long (os error 36)`** (exit 3; the
system's own words, with its own number on macOS and Windows) — the file name, often a long
`--query-name` in the table names, is longer than the file system holding it accepts (255 bytes
on ext4, 255 characters on FAT, exFAT, NTFS and HFS+). Shorten it. With **`caused by: too many
levels of symbolic links`**, the name is a symbolic link that leads, through others, back to
itself: remove the loop; with another cause, such as a permission error, the link cannot be
read. All are found before the analysis starts.

### Outputs that would overwrite each other

Every pair of outputs, and each output and the input, is checked before anything runs; the
message names both in words. See
[Choose and read outputs](/how-to/outputs/#outputs-that-would-overwrite-each-other).

**`'…' would be written twice: as … and as …`** — two outputs are the same file, possibly through
another spelling (`./x`, `d/../x`) or a symbolic link; the message names them, for example
`as the run report (--run-report) and as the positions table (--tsv-dir)`. Give each its own
name; when one is the run report, the hint suggests a free name for it.

**`… may be the same file as …`** — two names differ only in letter case, which macOS and
Windows treat as one file (or only in how their accented letters are encoded, which macOS
ignores), and the file system cannot tell yet whether they are one file: neither exists. DiMA
refuses them on every system so that a command never loses an output there. Rename one. When
only one of the two exists, or (on Linux and macOS) both exist as different files, they are two
files and nothing is refused.

**`output '…' (…) is the input file`** — an output would replace the input, which DiMA never
overwrites. Choose another output path. With `(read from standard input)`, standard input is
that file (`dima analyze < x.fasta -o x.fasta`).

**`'…' is given both as the --tsv-dir folder and as …`** — the folder of the tables and another
output have the same name. Rename one.

**`two outputs would go to standard output: … and …`** — `-`, `/dev/stdout` and `/dev/fd/1` are
all standard output, which can carry one output. Send the others to files. When one of them is
`the results (standard output by default)` — no `-o`, `--tsv-dir` or `--hcs-output` was given,
and `--run-report -` was — write the results to a file with `-o FILE`, or discard them with
`-o /dev/null -O json`.

**`standard output, where … would go, is the file '…', which is also written as …`** (or
`is the input file`) — standard output was redirected (`> file`, `>> file`) to a file that
another output replaces, or to the input. Redirect standard output elsewhere, or give the other
output its own file. Devices such as `/dev/null` and appending to a log are never refused. With
`dima cite`, `standard output, where the citation would go, is the input file '…'` means that
`>>` would append the citation to a `--from` file, which would then no longer be valid JSON.

**`… and … are the same file: … and … would both be written to it`** — two descriptors, such as
standard output and `/dev/fd/3`, lead to one file (`3>same.json >same.json`). Give each output
its own file.

**`… and … are the same pipe: … and … would both be written to it`**, **`… is the pipe '…',
which is also written as …`**, **`'…' and '…' lead to one pipe: …`**, **`/dev/fd/3 would be
written twice: …`** — two outputs go into one pipe, FIFO or socket (`-o /dev/fd/3 -O json
--hcs-output - 3>&1 | wc -c`, or a symbolic link to `/dev/stdout` next to `-`), where their bytes
would be mixed into one stream that neither format can read. Give each output its own file or
pipe.

**`output '…' (…) is the input pipe (read from standard input)`** — standard input comes from a
FIFO that an output also writes (`dima analyze -o p -O json < p`): DiMA itself would hold the
FIFO open for writing, so its input would never end. Write the output elsewhere.

The message `--run-report '…' is also given as '…'` of earlier versions is now one of the
messages above.

### Header and metadata options

**`metadata fields need a header format`** — `--metadata-fields` chooses among the fields named
by `--header-format`, so it needs that option too. Add `--header-format 'Accession|Host|Year'`
(your fields).

**`header format field 2 has no name`** — `--header-format` has an empty field, such as
`'A||C'`. Name every `|`-separated field.

**`header format field "…" appears more than once`** — two fields of `--header-format` share a
name. Give every field a distinct name.

**`metadata field "…" is not in the header format`** — `--metadata-fields` names a field that
`--header-format` does not; the hint lists the names it has. An empty `--metadata-fields ''`
gives `metadata field "" is not in the header format`: leave the option out to stratify by every
field. Names are compared after cleaning (white space trimmed, Unicode NFC), so ` Host ` and
`Host` are one name.

**`metadata field "…" is listed more than once`** — remove the repetition from
`--metadata-fields`.

### The conserved-sequence threshold

**`invalid HCS threshold "…": expected a decimal number such as 70 or 95.5`** — the value of
`--hcs-min-incidence` is not a plain decimal number: write `95`, not `95%`, and not an exponent
such as `1e2`.

**`invalid HCS threshold "…": a percentage cannot exceed 100`**, **`… at most 9 decimal places
are supported`** — write a value of at most 100, with at most 9 decimals.

**`HCS threshold …% is outside (50, 100]`** — the threshold must be above 50 and at most 100;
see [why](/explanation/conserved-sequences/#which-positions-qualify).

### `dima cite`

**`--from takes at most one results file and one conserved-sequences file`** — give the results
of one run and, optionally, the conserved sequences computed from them.

**`standard input is given twice to --from`** (since 1.2.0) — `-`, `/dev/stdin` and `/dev/fd/0`
all read standard input, which can be read once. Give the other
file by its name.

**`the conserved sequences in '…' were not computed from the results in '…': …`** (since 1.2.0)
— the two files come from different runs, so one methods text would describe two analyses. The
message says how: `their inputs differ (another checksum)`, `k is 9 in 'r.json' and 7 in
'h.json'` or `T is …`. Compute the conserved sequences from these results
with the command in the hint (`dima hcs -i RESULTS`), or cite each run on its own.

## Files and system resources (exit 3)

**`cannot open '…'`** (an input) or **`cannot create '…'`** (an output) with `No such file or
directory` or `Permission denied` — check the path and the permissions. For outputs, the
folder must exist (except `--tsv-dir`, which is created) and be writable, because each output
is first written to a temporary file next to it. Outputs, the run report first, are opened
before the analysis starts, so this error comes at once.

**`cannot create directory '…'`** — `--tsv-dir` could not be created: a part of the path is a
file, or a folder is not writable.

**`cannot read '…'`**, **`cannot write '…'`**, **`cannot flush '…'`**, **`cannot replace
'…'`**, **`cannot remove '…'`**, **`cannot inspect '…'`** — the operating system refused the
operation; the `caused by:` line says why and the hint suggests the fix: `No space left on
device` (free some disk space), `Disk quota exceeded` (free space or write elsewhere),
`File too large` (a file-size limit, see `ulimit -f`; raise it or compress the output),
`Read-only file system` (write the output elsewhere). After any of them, every output is as it
was before the run.

**`… is a directory`** (exit 2) — `-i` or the redirected standard input names a folder; give
the alignment file.

**`cannot start …`** (the thread that opens the input, the reader thread, the checksum thread,
the interrupt handler or a worker thread) — the system refused a new thread (`Resource temporarily unavailable`): a limit on
processes and threads (`ulimit -u`, a container's pids limit; on macOS, the limit of
threads per process) or a lack of memory. Use fewer threads (`-t`), stop other programs, or raise
the limit.

## The alignment (exit 4)

**`only N residue letters in the first … records`**, **`… contain both T (…) and U (…)`** or
**`the residue composition … does not identify the alphabet`** — automatic detection could not
decide between protein, DNA and RNA: too few letters, both T and U, or a composition that is not
clearly one of them. Say which with `-a protein`, `-a dna` or `-a rna`; see
[Set the alphabet](/how-to/analyse/#set-the-alphabet-if-needed).

**`the sequence has … alignment columns, but the first record has …`** — the sequences are not
aligned, or a record is truncated. DiMA needs a multiple sequence alignment: align the sequences
first (MAFFT, Clustal Omega and similar tools), and check the record and line given.

**`character '…' is not valid in a … alignment`** — a character that is not a residue, gap or
known symbol of the alphabet (see [Symbol tables](/reference/symbols/)). First check `-a`: a
protein alignment analysed with `-a dna` stops at its first amino acid that is not a nucleotide
letter (`character 'F' (0x46) is not valid in a dna alignment`). If the characters are expected
and rare, `--validation permissive` excludes the k-mers that contain them and reports them, and
`--validation report` lists every character in the file. See
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
header must have exactly the named number of `|`-separated fields. When every checked header
differs, the format itself is probably wrong: compare it with a header (`head -n 1`). Otherwise
fix the headers, or accept the mismatches with `--validation permissive` (their stratified fields
get the fill value). See
[Stratify by header metadata](/how-to/metadata/#headers-with-the-wrong-number-of-fields).

**`header is not valid UTF-8`** — convert the file to UTF-8 (`iconv -f LATIN1 -t UTF-8`), or use
`--validation permissive` to replace invalid bytes. Headers are checked only with
`--header-format`.

**`… exceeds the range of DiMA's counters`** — a count would overflow (more than about 18
billion billion sequences, or more than 4 billion distinct k-mers at one position or distinct
values of one field). Split the input into smaller alignments and analyse them separately.

## Results files (exit 4)

**`…: not a DiMA results file`** (from `dima view`, `dima hcs` or `dima cite --from`) — the file
is not the JSON `dima analyze` writes: an alignment given to `dima view` instead of
`dima analyze`, or another JSON document. Results compressed with a format DiMA recognises but
does not read (bzip2, xz, LZ4, zip, 7z, …) get the message below instead. Give the results JSON
(optionally `.gz` or `.zst`).

**`…: compressed with bzip2, which DiMA does not read results from`** (or `compressed with xz
(.xz)`, `a zip archive, which DiMA does not read results from`, …; since 1.2.0) — results are
read plain, gzip- or Zstandard-compressed. If the file
holds DiMA results, decompress it (`bzip2 -dk r.json.bz2`) or recompress it with gzip or zstd;
if it is an alignment, analyse it with `dima analyze`, which reads bzip2.

**`…: this is a DiMA conserved-sequences file, not a results file`** — `dima view` and `dima hcs`
read results; conserved sequences (`--hcs-output`, `dima hcs -o`) are given to `dima cite --from`
only.

**`'…' is a directory`** (exit 2) — give the JSON file, not its folder.

**`…: not a valid DiMA results file: EOF while parsing …`** — the file ends in the middle of the
JSON: it was cut short (a copy interrupted, or standard output of an interrupted run). Write it
again with `dima analyze`, or with `dima view` from a complete copy.

**`written with results schema …`** (or **`written with conserved-sequences schema …`**; from
`dima view`, `dima hcs` or `dima cite --from`) — the major number of the file's schema version
(before the first dot) is not `1`: it was written by a DiMA version with another major schema
version, or its version is not a plain number, such as `01.1.0`. Re-run the analysis, or use the version that wrote the file.

**`position N: … is given, but …`**, **`… is missing`** or another message with the hint `the
file was edited or damaged` (exit 4, from `dima view`, `dima hcs` or `dima cite --from`; the
desktop app shows the same message when it cannot open the file) — the
results contradict themselves, for example a similarity-weighted value at a position without
support, no such value where the run computed them (`run.parameters.similarity_weighted_entropy`
says whether it did), or `position N: similarity_weighted_entropy is …, above the observed
entropy …, which it cannot exceed`. DiMA never writes such a file; it was changed after `dima analyze` wrote
it. Run the analysis again; if you edit results by hand, keep the rules of the
[results reference](/reference/json/).

## Interruptions (exit 128 + N)

On the first interrupt, DiMA prints a notice naming it (not with `-q`), stops, removes the
temporary files of the outputs it had not finished, and ends with 128 + the signal number:

```text
cancelling... (press Ctrl-C again to stop immediately)
cancelling on SIGTERM... (a second signal stops immediately)
```

The first is for Ctrl-C on a terminal; the second names the signal another program sent, such
as a batch scheduler's SIGTERM at a time limit (a SIGINT sent where no terminal is attached reads
`cancelling on SIGINT...`, since 1.2.0). A second signal ends DiMA at once, as does the end of a
3-second grace period after the first. Then comes an error line, printed even with `-q`:

**`cancelled; no results were written`** — every file output is as it was before the run. Run
the command again.

**`cancelled; complete: …`** (since 1.2.0) — the interruption came while the finished outputs
were being moved into place: those named are complete, the others were not written.

**`…; standard output may hold a partial document`** (since 1.2.0) — the interruption came while
standard output (or another stream, which is then named) was being written, so whatever reads it
may have a cut-off document. Discard it; with `-o FILE` instead of `> FILE`, an interrupted run
never leaves a partial file.

The run report, when asked for, is written with status `cancelled` and lists the outputs that are
complete. See [Exit codes](/reference/exit-codes/#interruptions).

## Warnings and notes worth acting on

### No position is eligible

Both warnings mean that no position has support above the support threshold $T$, so the
summaries are empty — `no entropy summary (no eligible positions)` on the summary line — and,
with the default estimator, every entropy is the observed one. They have different causes and
fixes:

- **`threshold_exceeds_sequence_count`** — $T$ is not below the number of sequences $N$, so *no*
  position can have support above it, whatever k: a shorter k cannot help. Lower `-s` below $N$
  (at most $0.8\,N$ for the default estimator to correct the fully supported positions; see
  [Choose the support threshold](/how-to/analyse/#choose-the-support-threshold)), or add
  sequences.
- **`no_eligible_positions`** — $T$ is below $N$, but at every position fewer than $T$ sequences
  have a valid k-mer: gaps or ambiguity codes break the k-mers. Use a shorter `-k`, lower `-s`,
  or check why so many k-mers are invalid (gap-rich columns, ambiguity codes; `--validation
  report` counts the characters).

### Others

**`uncorrected_eligible_positions`** (since 1.2.0, default estimator only) — some eligible
positions kept their observed entropy, because their support is below $1.25\,T$: the paper's
correction needs at least five points, a minimum taken from DiMA v5.0.9. They are still eligible
and in the summaries, with values that are not corrected for sample size; the warning names
some, with their support. To have them corrected, give those positions at least $1.25\,T$
sequences, or lower `--support-threshold` (to at most 0.8 times their support), and report the
value used. Positions with a single distinct k-mer have 0 bits corrected or not, and are counted
apart. See [Diagnostics](/reference/diagnostics/#uncorrected_eligible_positions).

**`identifier_like_field`** — you are counting variants per accession or another unique
identifier; the output becomes very large. Use the `--metadata-fields` option the hint prints.

**`hcs_low_support_regions`** — some conserved regions rest on few sequences; see
[flagged regions](/how-to/conserved-sequences/#flagged-regions).

**`output_format_assumed`** (since 1.2.0) — the file of conserved sequences named by
`--hcs-output` or `dima hcs -o` has neither `.json` nor `.tsv` in its name, so DiMA wrote JSON,
the documented default. Rename it as the hint says (`regions.txt` → `regions.txt.json`), or, with
`dima hcs`, choose the format with `-O json` or `-O tsv`.

**`flag_has_no_effect`: `--similarity-weighted-entropy has no effect without results (JSON,
JSONL) or the positions table`** — the values of similarity-weighted entropy are written only
to the results and the positions table, and none of your outputs is one of them (for example,
only `-o variants.tsv --table variants`, or only `--hcs-output`). DiMA then does not compute
them and writes the other outputs as usual. Add `-o PATH.json`, or the positions table with
`-o PATH.tsv --table positions` or `--tsv-dir DIR`.

**`flag_has_no_effect`: `--hcs-min-incidence/--hcs-eligible-only have no effect without an HCS
output`** — no output holds conserved sequences. Add `--hcs-output PATH` or `--tsv-dir DIR`, or
write the table with `-o PATH.tsv --table hcs`.

**`flag_has_no_effect`: `--pretty has no effect without JSON output`** — `--pretty` indents JSON
only, and no output is JSON.

**`threads_exceed_cpus`** — more threads were asked for (`-t`, `RAYON_NUM_THREADS`) than this
process may use; they are used anyway, which slows the run down. Lower `--threads` to the CPUs
available.

**`threads_refused`** — the system started fewer worker threads than requested, so the run was
slower than it could be; the results are unaffected. Ask for fewer threads (`-t`) or raise the
limit on processes and threads (`ulimit -u`, a container's pids limit).

**`note: JSONL holds one position per line; provenance and summaries are in the JSON output`**
(with `-o x.jsonl` or `-O jsonl`) — JSON Lines carries the positions only. Keep the JSON results
(`-o x.json` in the same command) if you need the settings, the summaries or the provenance.

The command line's own warnings (`threads_exceed_cpus`, `threads_refused`, `flag_has_no_effect`,
`output_format_assumed`, `newer_schema`) are printed, and counted in the summary line of
`dima analyze`, but not stored in the results; see
[Diagnostics](/reference/diagnostics/#warnings-of-the-command-line). All diagnostics are
explained in [Diagnostics](/reference/diagnostics/).

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

## The desktop app

**"DiMA could not start: … If this mentions graphics, updating the graphics driver usually
helps. The command-line tool `dima` does not need a display."** — the app could not open its
window, usually because the graphics driver does not offer what it needs (or, on Linux, no
display is available, as over SSH). Update the graphics driver; on a machine without a display,
use the command-line tool.

**"Settings were reset to the defaults (…)"** — the settings file could not be read, was saved by
a different version of DiMA, or held values that are not valid, as the brackets say. The notice
shows at every start until you change a setting, which saves a new file. The settings are in
`settings.json` in `~/.config/dima-gui` (Linux), `~/Library/Application Support/dima-gui`
(macOS) or `%APPDATA%\dima-gui` (Windows).

**"Could not start …: the system refused a new thread (…). Nothing was written. Close other
programs or lower Worker threads under Advanced (Setup screen), then try again."** and **"Used N
of M worker threads: the system refused to start more. …"** — as `cannot start …` and
`threads_refused` above: close other programs, or lower **Worker threads** under **Advanced** on
the Setup screen, then try again. The results are the same with any number of threads.

**"Alphabet not detected: … Choose it under Alphabet below."** — on the Setup screen, the first
records do not tell protein from nucleotide sequences, for the reason the message gives: too few
residue letters ("only 3 residue letters in the first record; that is too few to tell protein
from nucleotide sequences"), both T and U ("the sequences contain both T (2) and U (2), so DNA
and RNA cannot be told apart"), or a composition that decides nothing ("the residue composition
of the first 10000 records (60000 letters) does not identify the alphabet"). Choose **Protein**,
**DNA** or **RNA** under **Alphabet**; the message then goes. See
[the alphabet](/explanation/input/#the-alphabet).

**"Not started: …"** — shown if an analysis is asked for while it cannot start (the **Analyze**
button is greyed out then, with the same reason under it, so this is rarely seen); the rest of
the message says why, and the fix is the same there:

- "Open an alignment first";
- "Choose the alphabet: …" (as above);
- "k = 27 exceeds the alignment length (6 columns): choose a k-mer length of at most 6", or "the
  default k for proteins (9) exceeds the alignment length (6 columns): …" when k was not set;
- "tick at least one Stratify box" (stratifying by header fields needs at least one);
- "the Fill value under Advanced must not be empty";
- "header format field 2 has no name", "header format field "Host" appears more than once"
  (name every field once in the header editor);
- "minimum coverage 2 is outside [0, 1]".

**"A file dialog is already open; choose a file there or cancel it first"**, **"An export is
already running; try again when it finishes"** — a shortcut or the command palette asked for a
second dialog or export while one is under way. Finish or cancel the first.

**"One file at a time: opening …"** — several files were dropped on the window; the app opens the
first. Drop or open the others one by one.

**"The system file dialog is not available here; using the built-in browser"** — the system's
file dialog could not be shown (on Linux, no desktop portal or `zenity`); the app's own file
browser opens instead. Nothing needs doing.

**"Nothing was saved: '…' asks for bzip2 compression, which DiMA does not write (DiMA compresses
with gzip or Zstandard: name it ….gz or ….zst)"** (or "…asks for two compressions…") — the file
name chosen for an export ends in a compression DiMA does not write, or in two. Name it as the
message suggests. A name without a known extension gets the export's own extension added instead
(see [Export](/how-to/desktop-app/#export)).

**"Nothing was saved: '…' is the alignment these results were computed from; choose another
name"** (or "…the results file these results were opened from", "…the alignment of the failed
run", "…the alignment on the Setup screen"; "…may be …" when the names differ only in letter
case, which macOS and Windows treat as one file) — the export would have replaced a file the app
is using. Choose another name.

**"'…' is compressed with bzip2, which DiMA does not read results from"** (hint: decompress it,
or recompress it with gzip or Zstandard, and open it again; alignments compressed with bzip2 open
as they are), **"'…' is compressed with xz (.xz), which DiMA does not read"**, **"'…' is a zip
archive, which DiMA does not read"** — the file opened is a results file in a compression the app
cannot read, or an archive. Decompress it (or extract the file from the archive) and open that.

**"Nothing was saved: other results were loaded while the dialog was open. Export again to save
the results now shown"** — the results shown changed between choosing an export and confirming
its file name, so the file would have held other results than those it was named for. Choose the
export again.

**"Nothing was saved: the chart was switched to similarity-weighted entropy while the dialog was
open. Export again to save the chart now shown."** (or "…switched to entropy…") — the chart showed
another metric when you chose the figure export than when you confirmed the file name, so the
file would have held the other metric's figure under a name chosen for this one. Choose the
export again; the save dialog then proposes the name of the chart now shown. See
[Export](/how-to/desktop-app/#export).

**"No positions are shown, so there is no chart to export (check the filters)"** — the filter bar
hides every position, so the chart is empty. Widen the filters (or press **Reset**) and export
again. When the filters were changed while the save dialog was open, the message starts with
"Nothing was saved:" and nothing is written.

**"… failed: …"** (for example "Entropy chart (PNG) failed: …") — an export could not be written;
the rest of the message is the same as the command line's (`cannot create …`, and so on, above).
**Copy details** copies the full report.

**"Internal error while** (what the app was doing)**. Nothing was written. Please report it (Copy
details)."** — a bug.
Press **Copy details** and include the text in a report on
[GitHub Issues](https://github.com/ShanWeera/dima-perf/issues/new/choose).

**"Value range reset to the full range of similarity-weighted entropy"** (or "…of
entropy") — you switched the chart's metric after narrowing the value range in the filter bar.
Each metric has its own scale, so the range starts over at the new metric's full range; narrow
it again if you need to. See
[Plot similarity-weighted entropy](/how-to/desktop-app/#plot-similarity-weighted-entropy).

**"Exported figures are now at least 720 × 320 px so the title, legend and axes
fit; yours was raised to W × H px"** (or "Exported figures are between 720 × 320 and 20000 ×
20000 px; yours was changed to W × H px") — the figure size saved in the settings, by an earlier
version or by hand, was outside the sizes exported figures can have. The app moved it into that
range once and saved it; nothing else changed. Nothing needs doing; to choose another size, set it
in **Preferences**. See [Open an alignment](/how-to/desktop-app/#open-an-alignment).

Dialogs ask before the app drops work: **"An analysis is running"** (**Cancel analysis and
open** stops it and opens the new file; nothing of the running analysis is kept), **"An export is
being saved"** (**Quit without saving it** quits now and the file is not written; keep DiMA open
to let the export finish) and **"Replace the file?"** (an export would replace an existing file).

**The desktop app does not open** on macOS or Windows — the packages are not code-signed yet; see
[first launch](/getting-started/install/#desktop-app).

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
[GitHub Issues](https://github.com/ShanWeera/dima-perf/issues/new/choose) with the full message,
the command, and if possible the input. When the program stopped unexpectedly (`this is a bug in
DiMA; please report it`), the message includes the version and platform, and `RUST_BACKTRACE=1`
adds a backtrace; otherwise add the output of `dima --version`.

For anything this page does not answer, see the [FAQ](/getting-started/faq/) or ask in
[GitHub Discussions](https://github.com/ShanWeera/dima-perf/discussions).
