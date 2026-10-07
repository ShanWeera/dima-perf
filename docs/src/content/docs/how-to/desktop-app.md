---
title: Use the desktop app
description: Analyse an alignment in the DiMA desktop app — open a file, set the header fields and options, run, explore the entropy chart, conserved regions and variants, and export results.
---

The desktop app, `dima-gui`, runs the same library as the command line: the same defaults,
validation rules, results and file formats. A results file written by `dima analyze` opens in the
app, and the app's exports read back in `dima view`, `dima hcs` and `dima cite`. To install it,
see [Install DiMA](/getting-started/install/#desktop-app).

The app has two screens, **Setup** and **Results**, switched from the top bar.

## Open an alignment

Open a file with **Open…**, by dropping it on the window, by typing or pasting its path, or from
the list of recent files. The app recognises by content whether it is an alignment (plain or
compressed) or a results file; a results file goes straight to the Results screen.

The app first inspects the beginning of an alignment — in well under a second, even for large
compressed files — and shows the detected alphabet, the alignment length, the default k, the
header fields with sample values, the compression, the size and the number of sequences
(estimated for large files). You can start the analysis right away; the run itself reads and
checks the whole file. With the alphabet on **Auto**, the run detects it again exactly as
`dima analyze` does, so the results and their provenance match the command line. If the file
changed after it was inspected, the run stops before counting, the app inspects it again and
asks you to review the settings.

If the operating system's file dialog is unavailable or does not respond (for example on a
remote desktop), the app offers its own file browser; you can choose it permanently in
**Preferences**, which also sets the size of exported figures.

## Describe the header fields

If the headers hold `|`-separated fields, tick **Stratify variants by FASTA header fields** to
open the header editor. It shows one row per field with three sample values. Name each field —
the paper's Accession, Strain, Species, Year, Geolocation and Host are offered, or a custom
name — and choose which fields to count variants by. Every field is counted by default, as on
the command line; a field whose sampled values are all different, such as an accession, is
marked *identifier-like*, and unticking it keeps the results small. **Paste format…** accepts a
format such as `Accession|Host|Country|Year`, and **Copy as `--header-format`** gives the
equivalent command-line option. See [Stratify by header metadata](/how-to/metadata/).

## Set the options

The main options are those of the paper: alphabet, k, the support threshold T, the query name and
the conserved-region threshold. An **Advanced** section holds the estimator, the validation mode,
the fill value for empty header fields, the minimum sample coverage and the number of worker
threads. Every option has a tooltip with its meaning and references. An option that differs from
its default is marked, and **Reset to defaults** restores them all. The app remembers the
options that do not depend on the file between sessions.

The worker threads default to all CPUs but one, and run at reduced priority, so the app stays
responsive during an analysis.

## Run

**Analyze** starts the run, with its progress and a **Cancel** button. If the alignment has a
problem, an error card says what failed and where — record, line, column, the header and the
character — and that no results were written; **Copy details** copies it for a report.

## Explore the results

The Results screen shows the run's facts across the top — sequences, positions, k, T, the mean,
lowest and highest entropy over eligible positions, the low-support counts — and a badge with the
number of warnings, which opens the diagnostics.

**Entropy chart.** One line along the alignment, encoded so that the meaning never depends on
colour alone:

- **solid blue**: positions with support above T, whose entropy the paper's estimator corrects;
- **dashed vermillion**: positions with support at or below T (LS, ELS), which the paper's
  estimator reports uncorrected (the observed entropy). With another estimator every position is
  estimated the same way, and the legend says "eligible" and "low support" instead;
- **hatched bands**: positions without usable support, which have no entropy — never drawn as 0;
- a horizontal line at the mean entropy of eligible positions.

Scroll to zoom about the pointer, drag to pan, Shift-drag to zoom to a region, double-click to
show everything; hovering a point shows its position, entropy, support and class, and clicking
selects it. Long alignments are drawn with M4 downsampling, which keeps every peak and trough.

**Conserved regions** appear as a strip under the chart, aligned with it at every zoom. The
threshold and the "eligible positions only" switch recompute them at once; flagged regions
(resting on positions with support ≤ T) are hatched and marked with an asterisk, and their
tooltip shows the minimum incidence, minimum support and 95% lower bound.

**Positions and variants.** The position table lists every shown position and can be sorted by
any column; the panels on the right describe the selected position (class, entropies and the
method that produced them, support, incidences, coverage), its motif composition, its variants
(searchable, filterable by incidence) and, for the selected variant, its counts per header value.
The filter bar restricts the view by position range, entropy range, motif class and support
class.

**Command palette.** Ctrl+K (⌘K on macOS) jumps to a position, finds the positions that carry a
k-mer, or runs a command. **Help ▸ Keyboard shortcuts** lists the others.

## Export

The export menu writes:

| Export | Same as |
|---|---|
| Results (JSON) — `.json`, `.json.gz` or `.json.zst` by name | `dima analyze -o` |
| Positions (JSON Lines) | `dima analyze -o x.jsonl` |
| Tables (TSV folder) | `dima analyze --tsv-dir` |
| Conserved regions (JSON or TSV), at the current threshold | `dima hcs` |
| Entropy chart (PNG or SVG), as shown | — |
| Run report | `dima analyze --run-report` |
| Methods & references, References (BibTeX) | `dima cite --from` with the results and the conserved regions |

Every file is written completely or not at all. **Run details** shows how the analysis ran
(equivalent command, threads, phase times, platform), and **Help ▸ Cite DiMA** gives the citation
of the paper and of this version, with **Copy citation** and **Copy BibTeX**.
