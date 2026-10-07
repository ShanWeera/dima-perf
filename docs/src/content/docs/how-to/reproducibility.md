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
values carry a small rounding error. On the bundled datasets it is in the last one or two of
the 17 significant digits. It grows with the number of sequences at a position, to at most the
11th digit at 17.5 million sequences. That is always at least $10^{10}$ times smaller than the
sampling error of an entropy estimate.
A future version may correct these last digits, so compare results across versions at about ten
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

The run report is written even when a run fails or is interrupted, and contains the equivalent
command with file names but without directories; it never contains user or host names,
directories or environment values. Its structure is in [JSON outputs](/reference/json/#run-report).
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
"Methodology". The schema version (`schema_version`) changes its major number when the meaning of
existing fields changes, and DiMA refuses to read results of another major version.

## References

[^ref]
