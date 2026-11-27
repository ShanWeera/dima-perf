---
title: Choose and read outputs
description: Write DiMA's results as JSON, JSON Lines or tidy TSV tables, compress them, convert between them, and read them safely in R, Python and jq.
---

`dima analyze` computes the results once and can write them in several forms in the same run.

| You want | Option | Notes |
|---|---|---|
| Everything, with provenance | `-o results.json` | The canonical output; the other forms can be made from it |
| One position per line, for streaming tools | `-o results.jsonl` | Positions only: no summaries or provenance |
| One table | `-o positions.tsv --table positions` | `positions`, `variants`, `metadata` or `hcs` |
| All four tables | `--tsv-dir tables` | `tables/<name>.{positions,variants,metadata,hcs}.tsv` |
| Conserved sequences | `--hcs-output hcs.json` (or `.tsv`) | See [Find conserved sequences](/how-to/conserved-sequences/) |
| How the run went | `--run-report report.json` | Times, memory, threads, checksums; see [Make results reproducible](/how-to/reproducibility/#keep-a-run-report) |
| Similarity-weighted entropy too | add `--similarity-weighted-entropy` | Written to the results and the positions table only; see [below](#similarity-weighted-entropy-in-the-outputs) |

Combine them freely:

```sh
dima analyze -i spike.fasta -o spike.json.zst --tsv-dir tables --hcs-output spike.hcs.tsv
```

## Rules

- **Format from the extension**, case-insensitive: `.json`; `.jsonl` or `.ndjson`; `.tsv` or
  `.tab`; each optionally followed by `.gz` or `.zst` to compress it. `-O json|jsonl|tsv` sets the
  format explicitly and must agree with the extension (`-O jsonl -o out.json` is an error, not a
  silent choice). A results name (`-o`) without a known extension needs `-O`:
  `-o /dev/null -O json`.
- **`-o -`** writes to standard output. Without `-o`, `--tsv-dir` and `--hcs-output`, JSON goes
  to standard output; with only `--tsv-dir` or `--hcs-output`, no JSON is written.
- **Compact JSON** by default; `--pretty` indents it, and JSON written to a terminal is indented
  automatically. The run report is always indented.
- **`--tsv-dir`** is created if needed (with any missing folder above it); its four files are
  replaced, other files in it are left alone. See [the table names](#the-table-names) below.
- **Checked before anything runs.** Every name, and every pair of outputs, is checked before the
  input is read, so a mistyped option never costs a long analysis. What is refused, with an
  error and exit status 2, is listed under [Names DiMA refuses](#names-dima-refuses) and
  [Outputs that would overwrite each other](#outputs-that-would-overwrite-each-other).
- **Safe replacement.** Each file — the results, every table, the conserved sequences and the
  run report — is written to a hidden temporary file next to it and renamed into place when
  complete, so an existing file is never left half-written: after an error or an interruption it
  is either as it was or complete. Standard output is different: it is written as the output is
  produced, so a run interrupted while writing it can leave a partial document there (DiMA then
  says so). Prefer `-o FILE` to `> FILE` when a partial file would be mistaken for a result.

### Names DiMA refuses

DiMA writes only gzip (`.gz`) and Zstandard (`.zst`). Since 1.2.0, a name that asks for anything
it would not honour is refused before anything runs, even with `-O`, instead of being written
under a name that misstates its bytes:

| Name | Why | Message |
|---|---|---|
| `mers.json.bz2`, `.xz`, `.lzma`, `.lz`, `.lz4`, `.Z`, `.br`, `.sz`, `.bgz` | A compression DiMA does not write | `'mers.json.bz2' asks for bzip2 compression, which DiMA does not write`<br/>`hint: DiMA compresses with gzip or Zstandard: name it mers.json.gz or mers.json.zst` |
| `.zip`, `.7z`, `.tar`, `.tar.gz`, `.tar.zst`, `.tgz`, `.tbz2`, `.txz` | An archive: DiMA writes one file per output | `… asks for a ZIP archive, which DiMA does not write` |
| `.zstd`, `.gzip` | A compression DiMA writes, under another name | the hint gives `.zst` or `.gz` |
| `mers.tsv.gz.zst` | Two compressions | `'mers.tsv.gz.zst' asks for two compressions (.gz, then .zst); DiMA applies one` |
| `results/` (ends in `/`, or `\` on Windows), `results/.`, `..` | A folder's name | `'results/' names a folder, not a file`<br/>`hint: give a file name, such as 'results/results.json'` |
| `results`, an existing folder | A folder | `'results' is a folder, not a file`<br/>`hint: give a file name, such as 'results/results.json'` |
| `--hcs-output mers.jsonl` | Conserved sequences are JSON or TSV | `'mers.jsonl' names JSON Lines, but conserved sequences are written as JSON or TSV`<br/>`hint: name it mers.hcs.json or mers.hcs.tsv (optionally followed by .gz or .zst)` |
| `--run-report run.tsv` (or `.jsonl`) | The run report is always JSON | `the run report is JSON, but 'run.tsv' names a TSV table`<br/>`hint: name it run.run-report.json (optionally followed by .gz or .zst)` |
| `--tsv-dir mers.json`, an existing file | `--tsv-dir` is a folder | `'mers.json' is a file, not a folder` |

A name DiMA suggests keeps the name you gave: it adds the format's extension to a name that names
none (`mers.hcs` → `mers.hcs.json`), and puts the output's own name in place of another format's
extension (`mers.jsonl` → `mers.hcs.json`, `run.tsv` → `run.run-report.json`), so it is never the
name the results of the same command usually have (`mers.json`, `run.json`).

A run report whose own name is refused is not written; any other refusal is still recorded in
the run report.

A file of conserved sequences (`--hcs-output`, or `-o` of `dima hcs`) whose name gives neither
`.json` nor `.tsv` gets JSON, the documented default, with a warning, because the name does not
say what the file holds:

```sh
dima analyze -i mers_spike_aa.fasta -o mers.json --hcs-output mers.hcs
```

```text
warning[output_format_assumed]: 'mers.hcs' does not name JSON or TSV, so the conserved sequences are written to it as JSON
  hint: name it mers.hcs.json or mers.hcs.tsv (optionally followed by .gz or .zst)
analysed 191 sequences, 1345 positions (1345 eligible) in 0.01 s; entropy mean 0.1261, min 0.0000, max 1.5878 bits; 44 conserved regions; 1 warning (see above)
```

The warning counts in the summary line and `-q` hides it; with `dima hcs`, `-O json` or
`-O tsv` chooses the format instead. Standard output (`-`) and other streams, such as
`/dev/stdout` or `/dev/null`, have no name to read, so they get JSON without a warning. The run
report is JSON whatever its name (other than the refused ones above).

### Outputs that would overwrite each other

Two outputs on one file, or an output on the input, are refused, with both named, before
anything runs. Two names are one file when they lead to it: spelled differently (`./x.json`,
`d/../x.json`) or through a symbolic link. Names that differ only in letter case are one file on
macOS and Windows (and names that differ only in how their accented letters are encoded, on
macOS), so they are refused on every system unless the file system shows they are two files:
when only one of them exists, or, on Linux and macOS, when both exist and are different files;
when neither exists yet, their folders are asked the same (`Out/r.json` and `out/r.json` are two
files where `Out` and `out` are two folders).
For example:

```text
error: 'mers.json' would be written twice: as the results (-o) and as the conserved sequences (--hcs-output)
  hint: give every output its own file
error: output 'mers_spike_aa.fasta' (the results (-o)) is the input file
  hint: choose another output path; the input is never overwritten
error: the results (-o) 'Mers.json' may be the same file as the conserved sequences (--hcs-output) 'mers.JSON'
  hint: names that differ only in letter case are one file on macOS and Windows; give every output its own file
```

The run report is checked first, against every other output — the four `--tsv-dir` tables
included — the `--tsv-dir` folder and the input, because it is written even when a run fails:
a report that would replace another file is refused and not written. (Before 1.2.0 a run
report named like one of the tables could replace it.)

Standard output takes one output: `-o -` and `--hcs-output -`, or `/dev/stdout`, together are
refused (`two outputs would go to standard output`). So is `--run-report -` alone, because the
results already go to standard output when no output is named:

```text
error: two outputs would go to standard output: the run report (--run-report) and the results (standard output by default)
  hint: the results go to standard output when neither -o, --tsv-dir nor --hcs-output is given: write them to a file with -o FILE, or discard them with -o /dev/null -O json
```

On Linux and macOS, standard input and standard output have no name, so DiMA compares the file
behind them with the files it replaces. `dima analyze < mers_spike_aa.fasta -O json -o mers_spike_aa.fasta`
is refused like `-i mers_spike_aa.fasta`, and so is writing a file that standard output also
goes to:

```text
error: standard output, where the conserved sequences (--hcs-output) would go, is the file 'mers.json', which is also written as the results (-o)
  hint: give every output its own file
```

That was `dima analyze -i mers_spike_aa.fasta -o mers.json --hcs-output - > mers.json`.
`dima cite --from mers.json >> mers.json` is refused the same way, since appending the citation
would make the results invalid JSON.

Two outputs into one pipe, FIFO or socket are refused too, because their bytes would be mixed in
it, and so is an output on the FIFO or pipe that standard input comes from (`-o p < p`), which
would keep the input from ever ending: `-o /dev/fd/3 -O json --hcs-output /dev/fd/3`, or `-o /dev/fd/3 -O json --hcs-output - 3>&1 |
wc -c`, or a symbolic link to `/dev/stdout` next to `-`:

```text
error: /dev/fd/3 and standard output are the same pipe: the results (-o) and the conserved sequences (--hcs-output) would both be written to it
  hint: give every output its own file
```

Devices and terminals take any number of outputs (`-o /dev/null -O json --hcs-output /dev/null`
is fine), appending standard output to a log (`-o /dev/stdout -O json >> run.log`) is fine, the
input may come from a pipe while the results go to another (`cat x.fasta | dima analyze > r.json`),
and a hard link to the input is another name, which DiMA does not refuse: writing it replaces
that name only. (Given to standard input, which has no name, the input is known by its inode
instead, so a hard link of it is refused as the input.)

### The table names

`--tsv-dir DIR` writes `DIR/NAME.positions.tsv`, `NAME.variants.tsv`, `NAME.metadata.tsv` and
`NAME.hcs.tsv`. `NAME` is the [query name](/reference/glossary/#query-name) made safe for file
names: characters other than Unicode letters, digits, `.`, `-` and `_` become `_`, leading and
trailing `.` and `_` are removed, and an empty name becomes `dima`.

| Command | Query name | Tables |
|---|---|---|
| `dima analyze -i mers_spike_aa.fasta --tsv-dir tables` | the input file's name without its FASTA and compression extensions | `tables/mers_spike_aa.*.tsv` |
| `dima analyze -i mers_spike_aa.fasta --query-name "MERS spike" --tsv-dir tables` | `--query-name` | `tables/MERS_spike.*.tsv` |
| `dima analyze --tsv-dir tables < mers_spike_aa.fasta` (also `-i /dev/stdin`, `-i <(...)`) | none | `tables/dima.*.tsv` |
| `dima view -i mers.json --tsv-dir tables` | the query name in the results | `tables/mers_spike_aa.*.tsv` (`Unknown_Protein.*.tsv`, `Unknown_DNA.*.tsv` or `Unknown_RNA.*.tsv` for results analysed from standard input) |

`dima view` reads the query name from the results, so it checks the table names against its
other outputs once it has read them, before it writes anything.

## Compression

`.gz` (gzip) and `.zst` (Zstandard) outputs are compressed as they are written. JSON results
compress very well — the MERS-CoV sample's 986 KB of JSON become 41 KB as `.json.zst` and 43 KB
as `.json.gz` — and every DiMA command reads compressed results directly (`dima view`, `dima hcs`,
`dima cite --from`, the desktop app). R and pandas read `.gz` tables directly; `.zst` needs the
`zstandard` Python package or `zstd -d` first.

## Convert results

`dima view` writes existing results in another form, without reanalysing:

```sh
dima view -i spike.json.zst -o spike.jsonl                       # JSON Lines
dima view -i spike.json.zst -o variants.tsv --table variants      # one table
dima view -i spike.json.zst --tsv-dir tables                      # all four tables
dima view -i spike.json.zst --pretty | less                       # readable JSON
```

The `hcs` table written by `dima view` uses the default conserved-sequence settings; use
`dima hcs` for others.

## Similarity-weighted entropy in the outputs

With `--similarity-weighted-entropy` (see
[Analyse an alignment](/how-to/analyse/#add-similarity-weighted-entropy-optional)), the values of
[similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) are
written to these outputs only:

| Output | What it holds |
|---|---|
| Results JSON (`-o x.json`) | Three keys per position, `similarity_weighted_entropy`, `normalised_observed_entropy` and `normalised_similarity_weighted_entropy`; the summary `similarity_weighted_entropy_summary`; the option, in `run.parameters.similarity_weighted_entropy` |
| JSON Lines (`-o x.jsonl`) | The three keys per position |
| `positions` table | Three columns of the same names, the last three of the table |

The `variants`, `metadata` and `hcs` tables and the conserved sequences do not hold them. If
those are all you write, DiMA warns (`flag_has_no_effect`) and does not compute the values.
Without the option the three keys and columns are still there, as `null` (`NA` in the table),
and the summary is `null`, so a script can rely on the same layout either way.

The positions table of the MERS-CoV spike sample, for three positions:

```sh
dima analyze -i mers_spike_aa.fasta --similarity-weighted-entropy -o mers.swe.json --tsv-dir tables
cut -f 1,3,6,13-15 tables/mers_spike_aa.positions.tsv | awk 'NR == 1 || $1 == 7 || $1 == 25 || $1 == 26'
```

```text
position	support	observed_entropy	similarity_weighted_entropy	normalised_observed_entropy	normalised_similarity_weighted_entropy
7	170	0.0	0.0	NA	NA
25	170	1.4223886721629135	0.42754448388901667	0.4281816558136978	0.12870371413126977
26	170	1.5433072491420126	0.4548302065456951	0.4461158419283731	0.1314754146592604
```

Position 7 has a single variant, so its similarity-weighted entropy is 0 and its two normalised
values are undefined (`NA`). A position without support has `NA` in all three columns.

The summary covers the eligible positions (support above the support threshold), like the
top-level entropy summary, and gives the mean observed entropy of the same positions for
comparison:

```sh
jq '.similarity_weighted_entropy_summary' mers.swe.json
```

```json
{
  "position_count": 1345,
  "average_entropy": 0.03626017430990571,
  "lowest_entropy": {
    "position": 7,
    "entropy": 0.0,
    "tied_position_count": 455
  },
  "highest_entropy": {
    "position": 26,
    "entropy": 0.4548302065456951,
    "tied_position_count": 1
  },
  "average_observed_entropy": 0.12068058783907992
}
```

Results written by dima 1.0.0 have no similarity-weighted values: `dima view` writes them as
`null` (`NA` in the positions table) and the option as off, and says so in a note
([Troubleshooting](/how-to/troubleshooting/#results-written-by-another-dima-version)).

## Read the tables in R and Python

The tables use `NA` for undefined or not requested values, and only the `positions` table has
any. Text columns
can contain strings that R and pandas would otherwise read as missing: the protein 2-mer `NA`,
the 3-mer `NAN`, or header values such as `nan` or `None`. Tell the reader exactly what is
missing:

**R ([readr](https://readr.tidyverse.org/))**

```r
library(readr)
positions <- read_tsv("tables/spike.positions.tsv", na = "NA")
variants  <- read_tsv("tables/spike.variants.tsv",  na = character())
metadata  <- read_tsv("tables/spike.metadata.tsv",  na = character(),
                      col_types = cols(value = col_character()))
hcs       <- read_tsv("tables/spike.hcs.tsv",       na = character())
```

With base R, use `read.delim(file, na.strings = "NA")` for positions and
`read.delim(file, na.strings = character(0), colClasses = c(value = "character"))` for metadata.

**Python ([pandas](https://pandas.pydata.org/))**

```python
import pandas as pd

positions = pd.read_csv("tables/spike.positions.tsv", sep="\t",
                        na_values=["NA"], keep_default_na=False)
variants = pd.read_csv("tables/spike.variants.tsv", sep="\t", keep_default_na=False)
metadata = pd.read_csv("tables/spike.metadata.tsv", sep="\t", keep_default_na=False,
                       dtype={"value": str})
hcs = pd.read_csv("tables/spike.hcs.tsv", sep="\t", keep_default_na=False)
```

Then, for example, the most diverse eligible positions and their dominant variant:

```python
eligible = positions[positions["low_support"].isna()]
top = eligible.nlargest(10, "entropy")[["position", "support", "entropy"]]
index = variants[variants["motif_short"] == "I"][["position", "sequence", "incidence"]]
print(top.merge(index, on="position"))
```

## Plot entropy along the alignment

The desktop app exports its entropy chart as PNG or SVG. From the tables, plot `entropy` against
`position`. Positions without support have no entropy (`NA`): keep them as gaps in the line, not
zeros, because a missing value is not a conserved position. Both plotting functions below leave a
gap at every `NA`.

**R (base graphics)**

```r
positions <- read.delim("tables/spike.positions.tsv", na.strings = "NA")
plot(positions$position, positions$entropy, type = "l",
     xlab = "Position (alignment column)", ylab = "Entropy (bits)")
```

**Python ([matplotlib](https://matplotlib.org/))**

```python
import matplotlib.pyplot as plt
import pandas as pd

positions = pd.read_csv("tables/spike.positions.tsv", sep="\t",
                        na_values=["NA"], keep_default_na=False)
fig, ax = plt.subplots(figsize=(10, 3))
ax.plot(positions["position"], positions["entropy"], linewidth=0.8)
ax.set_xlabel("Position (alignment column)")
ax.set_ylabel("Entropy (bits)")
fig.savefig("entropy.svg", bbox_inches="tight")
```

Only eligible positions carry the corrected entropy. To show the others, mark the positions whose
`low_support` is not `NA` (NS, LS or ELS).

To add the desktop app's dashed *eligible mean* line, draw a horizontal line at the
[eligible mean](/reference/glossary/#eligible-mean), the mean entropy of the eligible positions
(those whose `low_support` is `NA`). It equals `average_entropy` in the results JSON:

```r
abline(h = mean(positions$entropy[is.na(positions$low_support)]), lty = 2)
```

```python
eligible_mean = positions.loc[positions["low_support"].isna(), "entropy"].mean()
ax.axhline(eligible_mean, linestyle="--", linewidth=0.8, color="grey")
```

Add the line before `fig.savefig` in Python, after `plot` in R. For the HIV-1 *pol* protein data
set (`tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2`, default options) it is at 1.9072 bits; the
mean of every non-`NA` value of the `entropy` column, LS positions included, would be 1.5708.

To plot similarity-weighted entropy instead, run the analysis with
`--similarity-weighted-entropy` and plot the `similarity_weighted_entropy` column in place of
`entropy`; it too is `NA` at positions without support.

## Read the JSON

With `jq`:

```sh
jq '.average_entropy, .highest_entropy' spike.json
jq '.results[24].diversity_motifs[] | {sequence, count, motif_long}' spike.json
jq -r '.run.diagnostics[] | "\(.code): \(.message)"' spike.json
zstd -dc spike.json.zst | jq '.run.parameters'
```

With Python, the JSON results are one document; JSON Lines can be read one position at a time:

```python
import json

with open("spike.json") as f:
    results = json.load(f)
print(results["average_entropy"], results["run"]["input"]["checksum"]["value"])

with open("spike.jsonl") as f:
    for line in f:
        position = json.loads(line)
        if position["entropy"] is not None and position["entropy"] > 1:
            print(position["position"], position["entropy"])
```

Python's `json` reads every number exactly as written. The structure of every field is in
[JSON outputs](/reference/json/) and the column definitions in [TSV tables](/reference/tsv/).
