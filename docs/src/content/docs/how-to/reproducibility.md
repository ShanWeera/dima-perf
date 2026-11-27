---
title: Make results reproducible
description: What DiMA guarantees about identical results, what it records about each run, and how to archive, verify and cite an analysis so that others can reproduce it.
---

Reproducible analyses keep track of how every result was produced, archive the exact software
version, and avoid untracked randomness [@sandve2013, Rules 1, 3 and 6]. DiMA is built so that
this takes little effort.

## What DiMA guarantees

The same alignment analysed with the same options and the same DiMA version gives
**byte-identical results** on Linux, macOS and Windows, on any processor and with any number of
threads.

A copy of the alignment that is wrapped differently, in another case, with LF or CRLF line
endings, compressed, or read from standard input gives identical **analysis values**: the
positions, variants, summaries and tables. The `run` record describes the file as it was read,
so it differs: `run.input` (its name, size, checksum and compression), and `run.validation` for
a lower-case copy (the characters seen, and a note that they were folded). The default
`query_name` comes from the file name (`Unknown Protein` and the like for standard input); give
`--query-name` to fix it.

DiMA uses no random numbers — the paper's random entropy correction is replaced by its exact
expectation (see [Entropy and its correction](/explanation/entropy/#the-exact-expectation)) — and
the results contain no timestamps, host names, paths or thread counts. These guarantees are
checked by the test suite on every change [@taschuk2017, Rule 10].

Byte-identical is not the same as exact. Like any floating-point program, DiMA rounds, and its
values carry a small rounding error. On the paper's four datasets in the repository (up to 3,756 sequences) it is in the last
one or two of the 17 significant digits (the last three or four for the opt-in similarity-weighted entropy). It
grows with the number of sequences at a position, to at most the 11th digit at 17.5 million
sequences (the 10th for similarity-weighted entropy). That is always at least $10^{10}$ times
smaller than the sampling error of an entropy estimate ($10^9$ times for
similarity-weighted entropy).
A future version may correct these last digits, so compare results across versions at about nine
significant digits. Results from the same version match exactly. The measurements are in
[Numerical accuracy](/explanation/numerical-accuracy/).

## What the results record

Every results file carries its own provenance in `run` (see [JSON outputs](/reference/json/#run)):

- the DiMA version (`run.tool.version`) and the results schema version;
- every parameter with its value and origin — set by you, the documented default, or derived from
  the data (the alphabet, the default k, the query name);
- the input's file name, size, compression and **SHA-256 checksum** of its stored bytes;
- how the alphabet was decided, what validation found, how header values were cleaned;
- every diagnostic;
- the references of the methods used.

To check that a results file belongs to an alignment:

```sh
sha256sum alignment.fasta.zst
jq -r '.run.input.checksum.value' results.json
```

## Keep a run report

Execution details — when the run started, how long each phase took, the threads and where their
number came from, CPU time, peak memory, the platform, and the size and checksum of every output —
vary from run to run, so they are kept out of the results. Ask for them in a separate file:

```sh
dima analyze -i alignment.fasta.zst -o results.json.zst --run-report run-report.json
```

The run report is written even when a run fails or is interrupted — after an interruption it
lists the outputs that are complete — and contains the equivalent command with file names but
without directories; it never contains user or host names, directories or environment values.
It is checked and opened before anything else, so a report that would replace another output or
the input is refused and not written, and one that cannot be created stops the run at once
([Choose and read outputs](/how-to/outputs/#outputs-that-would-overwrite-each-other)). Its
structure is in [JSON outputs](/reference/json/#run-report).
The desktop app saves a report in the same format (**Export ▸ Run report…**, and **Save run
report…** on a failed run), with two differences: CPU time and peak memory are not recorded
(`null`), because the app's own work runs in the same process, and a cancelled run has no
report.

## Archive an analysis

To let others repeat an analysis exactly, archive together:

1. the alignment (or its accession list and the alignment procedure), with its SHA-256;
2. the results JSON (it records every parameter, so the command can be rebuilt from it) and,
   optionally, the run report;
3. the DiMA version: the release archive, or `cargo install --locked --git
   https://github.com/ShanWeera/dima-perf --tag vX.Y.Z dima` for a source build.

Release archives come with `SHA256SUMS` and build-provenance attestations, so the binary can be
verified later (see [Install DiMA](/getting-started/install/#verify-the-download)).

## Cite the analysis

```sh
dima cite --from results.json
```

prints a methods paragraph for that run — alphabet, number of sequences and positions, $k$, $T$,
estimator — followed by the references of the methods it used, including the exact DiMA version.
See [Citing DiMA](/reference/citing/).

## Comparing versions

Results of different DiMA versions can differ where a new version fixes or adds something; the
[changelog](/reference/changelog/) lists every change that can alter a result under
"Methodology". The schema version (`schema_version`) changes its minor number when keys are added
and its major number when the meaning of existing fields changes, and DiMA refuses to read
results of another major version.

Results schema 1.1.0 added the keys of
[similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in): three
per position, `similarity_weighted_entropy_summary` and the parameter
`run.parameters.similarity_weighted_entropy`. Results written by dima 1.0.0 (schema 1.0.0) are
read as runs without that option: `dima view`, `dima hcs`, `dima cite --from` and the desktop app
read them as before, and `dima view` writes the new keys as `null` (`NA` in the positions table)
and the option as off, with a note that says so. To validate a 1.0.0 file, use the 1.0.0 schema:
`schemas/results.schema.json` of release 1.0.0 (tag `v1.0.0`), the schema the documentation of
version 1.0 describes. The schema published at the address in its `$id` is now 1.1.0: it
requires `schema_version` 1.1.0 and the keys 1.1.0 added, so a 1.0.0 file does not validate
against it.

Results written by a newer 1.x version are read too, keeping only the keys this version knows;
`dima view` warns about it (`newer_schema`), the desktop app opens such a file with the same
warning (a version it does not recognise is quoted in both), and a value this version does not
know stops the reading. `dima hcs` and `dima cite --from` read such files without a warning, and
leave out what this version does not know: dima 1.0.0's `dima cite --from`, for example, writes
no sentence for similarity-weighted entropy, although the run computed it and the references list
Wu 2025. Read results with the version that wrote them, or a newer one. See
[Troubleshooting](/how-to/troubleshooting/#results-written-by-another-dima-version).

## Compare with DiMA v5.0.9

Results of the earlier implementation, DiMA v5.0.9 (`dima-cli` 5.x on PyPI), share many key
names with these results, but some keys with the same name hold a different quantity. Compare
like with like:

| Key | DiMA v5.0.9 | This DiMA | Compare v5.0.9's value with |
|---|---|---|---|
| `average_entropy` | The mean over **all** positions, those without support counted as 0 bits | The [eligible mean](/reference/glossary/#eligible-mean): positions with support above $T$ only | `all_positions_summary.average_entropy`, which follows v5.0.9's rule |
| `highest_entropy` | The **last** position with the highest entropy, its position counted **from 0** | The **first** eligible position with the highest entropy, counted from 1, with `tied_position_count` | `all_positions_summary.highest_entropy` (the first maximum over all positions, from 1) |
| `entropy` at positions without support (NS) | `0.0` | `null`: there is nothing to measure | — |
| `diversity_motifs` at NS | `null` | `[]` | — |

For example, the 26-sequence alignment of v5.0.9's own tests, with v5.0.9's `-s 30`:

```sh
curl -fLO https://raw.githubusercontent.com/ShanWeera/dima-perf/master/tests/data/dima-v5.0.9/input.afa.bz2
dima analyze -i input.afa.bz2 -s 30 -o v509cmp.json
jq -c '{average_entropy, all: .all_positions_summary | {average_entropy, highest_entropy}}' v509cmp.json
```

```json
{"average_entropy":null,"all":{"average_entropy":0.06854034285524646,"highest_entropy":{"position":180,"entropy":1.3921472236645345,"tied_position_count":8}}}
```

v5.0.9's own output for the same run (`tests/data/dima-v5.0.9/basic_output.json.bz2`) has
`average_entropy` 0.06854034285524647 — the same mean, but for a rounding difference in the last
digit — and `highest_entropy` at position 186, counted from 0: position 187 counted from 1, the
last of the eight tied positions 180 to 187. This DiMA's `average_entropy` is `null`, because with $T = 30$ above the 26
sequences no position is eligible. On the HIV-1 *pol* DNA alignment, the eligible mean is 5.3504 bits, and the
mean by v5.0.9's rule is 3.4425 bits.

Where positions are eligible, the entropies themselves differ too: v5.0.9 corrects with random
subsamples, which change from run to run, and this DiMA computes the expected value of that
correction exactly (see [Entropy and its correction](/explanation/entropy/#the-exact-expectation)).
[Differences from DiMA v5.0.9](/explanation/differences/) lists every difference.

## Reproduce the paper's values

The script that compares DiMA with the values published in the paper uses the paper's datasets,
which are in the repository, so it needs a clone; it runs any `dima` binary, such as an installed
one:

```sh
git clone https://github.com/ShanWeera/dima-perf.git
cd dima-perf
scripts/validate_paper.sh "$(command -v dima)"
```

It prints a Markdown table of each published value next to DiMA's, with a note where they differ
and why. Without an argument it builds and uses `target/release/dima`. It needs `python3`
(standard library only). [Comparison with the papers](/explanation/paper-comparison/) explains
every row.

## References

[^ref]
