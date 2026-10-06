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

Combine them freely:

```sh
dima analyze -i spike.fasta -o spike.json.zst --tsv-dir tables --hcs-output spike.hcs.tsv
```

## Rules

- **Format from the extension**, case-insensitive: `.json`; `.jsonl` or `.ndjson`; `.tsv` or
  `.tab`; each optionally followed by `.gz` or `.zst` to compress it. `-O json|jsonl|tsv` sets the
  format explicitly and must agree with the extension (`-O jsonl -o out.json` is an error, not a
  silent choice). A name without a known extension needs `-O`:
  `-o /dev/null -O json`.
- **`-o -`** writes to standard output. Without `-o`, `--tsv-dir` and `--hcs-output`, JSON goes
  to standard output; with only `--tsv-dir` or `--hcs-output`, no JSON is written.
- **Compact JSON** by default; `--pretty` indents it, and JSON written to a terminal is indented
  automatically.
- **Safe replacement.** Each file is written to a temporary file next to it and renamed into
  place when complete, so an existing file is never left half-written. Writing the same file
  twice in one command (for example `-o x.json --hcs-output x.json`), or writing over the input,
  is refused before the analysis starts.
- **`--tsv-dir`** is created if needed; its four files are replaced, other files in it are left
  alone. Their names start with the query name (`--query-name`, by default the input file name
  without extensions).

## Compression

`.gz` (gzip) and `.zst` (Zstandard) outputs are compressed as they are written. JSON results
compress very well — the MERS-CoV sample's 830 KB of JSON become about 40 KB as `.json.zst` —
and every DiMA command reads compressed results directly (`dima view`, `dima hcs`,
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

## Read the tables in R and Python

The tables use `NA` for undefined values, and only the `positions` table has any. Text columns
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
