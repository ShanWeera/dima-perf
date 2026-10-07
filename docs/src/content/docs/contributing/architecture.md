---
title: Architecture
description: How the DiMA code base is organised — the library, the command line and the desktop app — and how an analysis flows through the library's modules.
---

DiMA is a Rust workspace of three crates with one rule: **every computation, default, validation
rule, citation and output format lives in the library**; the command line and the desktop app
only collect options, show progress and present results. Results therefore cannot differ between
the two front ends.

| Crate | Path | Contents |
|---|---|---|
| `dima-core` | `crates/dima-core` | The library `dima_core`: input, analysis, statistics, outputs, provenance, citations. No terminal or GUI dependencies, and no `unsafe` code (`#![forbid(unsafe_code)]`) |
| `dima` | `crates/dima` | The command-line program `dima` |
| `dima-gui` | `crates/dima-gui` | The desktop app `dima-gui` (egui/eframe) |
| `xtask` | `xtask` | `cargo xtask gen`: generated documentation and citation files |

Only 64-bit targets are supported (counts and offsets are 64-bit); building for a 32-bit target
fails with a clear message.

## The library

The modules are layered: each uses only modules of the layers above it.

| Layer | Module | Responsibility |
|---|---|---|
| foundations | `error` | The error type, with locations (record, line, column, byte) and hints, and its exit classes |
| | `citations` | The registry of every reference and its text, BibTeX, RIS and CFF forms |
| | `diagnostics` | Typed, aggregated diagnostics with stable codes |
| | `config` | Parameters, their defaults and validation; exact decimal percentages |
| | `numeric` | Compensated summation, binomial walks, Clopper–Pearson bounds, least squares |
| | `human` | Human-readable numbers for messages |
| | `control` | Cancellation flag and progress counters shared with the front ends |
| data | `alphabet` | Alphabet detection (Easel port) and per-byte symbol classes |
| | `model` | The typed results and their JSON representation |
| reading | `input` | Opening a source once, checksum, compression detection, splitting into batches of raw records |
| | `header` | Header formats, value cleaning, interning, metadata diagnostics |
| | `normalise` | Parallel conversion of raw records into fixed-width rows of canonical residues |
| | `validation` | The first-batch probe shared by the analysis and the desktop app's pre-check |
| statistics | `entropy` | Plug-in, DiMA, Miller–Madow and Chao–Wang–Jost estimators; sample coverage |
| | `stats` | Support labels, motif classes and incidences |
| | `position` | One position's result from its counts |
| | `summary` | Run summaries |
| | `hcs` | Historically conserved sequences |
| | `provenance` | The deterministic `run` record and the diagnostics of a run |
| | `methods` | The methods paragraph and reference list of `dima cite --from` |
| engine | `engine` | The streaming k-mer counter |
| orchestration | `analysis` | `analyze()`: wires the steps together |
| outputs | `output` | JSON, JSON Lines and TSV writers, the atomic file `Sink`, `read_results()` |
| | `run_report` | The run report |

### How an analysis flows

1. **Open** (`input`). The source — file, pipe or standard input — is read once. The stored bytes
   are copied to a checksum thread (SHA-256); the format is recognised from the first bytes and
   decompressed; a byte-order mark is skipped. A reader thread splits the text into records
   (`seq_io`) and fills batches of at most 10,000 records or 64 MiB, one batch ahead of the
   workers.
2. **Probe** (`validation`, `alphabet`, `header`). The first batch decides the alphabet, the
   symbol table, the default k and the header statistics.
3. **Normalise** (`normalise`). Records are converted in parallel into rows of canonical residues,
   one byte per alignment column, with every excluded symbol marked; errors carry their record,
   line, column and byte.
4. **Count** (`engine`). Positions are split into contiguous shards, each owned by one worker.
   For each row, a rolling hash (Karp–Rabin, modulo $2^{61}-1$) identifies the k-mer at each
   position in O(1); a per-position hash table, verified byte by byte, assigns exact k-mer
   identifiers and counts them, with header-value counts alongside. Nothing proportional to
   sequences × positions is stored.
5. **Finalise** (`position`, `entropy`, `stats`). Positions are finalised in parallel from their
   counts: support class, entropies, coverage, motifs, incidences, metadata.
6. **Summarise** (`summary`, `provenance`). Summaries are reduced in position order, and the
   provenance and diagnostics are assembled.
7. **Write** (`output`, `hcs`). The front end writes the requested outputs through the atomic
   `Sink` (temporary file in the destination directory, sync, rename).

Determinism comes from this structure: no randomness, logarithms from `libm`, compensated sums in
a fixed order, variants sorted by count and bytes, and k-mer identifiers that never leave the
engine. Thread count and batch size change only the speed.

## The command line

`crates/dima/src/cli`: `args` (the command-line syntax, with clap), `help` (help texts, with
references from the registry), `analyze`, `view`, `hcs`, `cite` (the subcommands), `outputs`
(planning and checking every output before the analysis starts), `threads` (the default thread
count from the scheduler's grant), `signals` (interruptions, cleanup and exit by signal),
`progress` (bars and milestones), `resources` (peak memory and CPU time) and `ui` (messages on
standard error). Platform-specific `unsafe` code — signal handling, host name, resource usage — is
confined to this crate.

## The desktop app

`crates/dima-gui/src`: `app` holds the state machine — a pure
`update(&mut AppState, Msg) -> Vec<Effect>` that turns events into state changes and side
effects —, `state` the plain-data application state, `views` the two screens (setup and results),
`panels` the pieces they are made of, `charts` the entropy chart's model, downsampling (M4) and
rendering (wgpu for the screen, SVG and PNG for export), `theme` the design tokens, `workers` the
background jobs (inspection, analysis, conserved regions, export, file dialogs), `persist` the
settings and recent files, and `report` the run report. Every worker calls the library; none
re-implements a computation.

## Supporting files

| Path | Contents |
|---|---|
| `schemas/` | JSON Schemas of the results, conserved-sequence and run-report documents |
| `scripts/reference/` | The independent reference implementation (Python, standard library only) |
| `tests/data/`, `tests/golden/` | Test datasets with provenance; expected outputs compared byte for byte |
| `packaging/` | The Bioconda recipe template |
| `docs/` | This documentation site |
