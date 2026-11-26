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
**Preferences**, which also sets the size of exported figures: at least 720 × 320 px, so the
title, legend and axes always fit, and at most 20,000 × 20,000 px. A smaller size saved by an
earlier version is raised to these minimums when this version first starts, with a note saying
so; a size above the maximum, possible only in a settings file edited by hand, is lowered to it
in the same way.

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
the fill value for empty header fields, the minimum sample coverage, similarity-weighted entropy
and the number of worker threads. Every option has a tooltip with its meaning and references. An
option that differs from its default is marked with a dot, whose tooltip names the default, and
**Reset to defaults** restores them all. The Advanced section starts closed, and the app
remembers whether you left it open, also after a restart. Its title counts the options inside it
that differ from their defaults, for example **Advanced (2 changed)**, so a changed option is
seen without opening it. The app remembers the options that do not depend on the file between
sessions.

The worker threads default to all CPUs but one, and run at reduced priority, so the app stays
responsive during an analysis.

### Similarity-weighted entropy

[Similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) is an
extra measure of diversity that is not part of the DiMA method, so it is off by default. It
counts two variants that differ at one residue as less diverse than two unrelated variants;
Shannon entropy counts them alike. To compute it:

1. Click **Advanced** to open the section.
2. Tick **Compute similarity-weighted entropy (Wu 2025)**, in the row named
   **Similarity-weighted**. A dot appears after the row's name (its tooltip: *Changed from the
   default (default: off)*), and the section's title reads **Advanced (1 changed)** from then
   on, also when it is closed.
3. Press **Analyze**.

The run then computes, at every position, the similarity-weighted entropy and the two
[normalised entropies](/reference/glossary/#normalised-entropy-h_n-and-h_nsim): each entropy
divided by log₂ D, the largest observed entropy possible with the position's D
[distinct k-mers](/reference/glossary/#distinct-k-mers-d). The app remembers the choice for the
next analyses, also after a restart, until you untick it or press **Reset to defaults**. **Run
details** says *Similarity-weighted entropy: computed* or *not computed*, also for results opened
from a file; a file written by dima 1.0.0 reads as *not computed*.

Without the option nothing of it is computed and the Results screen shows nothing of it; the
exported results hold `null` (`NA` in the positions table) where the values would be.

The app saves this choice in its settings only while the option is ticked, so the settings of
anyone who never ticks it stay readable by dima 1.0.0. If you go back to dima 1.0.0 with the
option ticked, 1.0.0 cannot read the saved settings: it starts with its default options and shows
*Settings were reset to the defaults (the saved values were not valid).* at every start, until
you change any setting in it, which saves its own settings in place of these.

## Run

**Analyze** starts the run, with its progress and a **Cancel** button. If the alignment has a
problem, an error card says what failed and where — record, line, column, the header and the
character — and that no results were written; **Copy details** copies it for a report.

## Explore the results

The Results screen shows the run's facts across the top — sequences, positions, k, T, the mean,
lowest and highest entropy over [eligible positions](/reference/glossary/#eligible-position), the
low-support counts and, when the run computed it, the similarity-weighted mean — and a badge
with the number of warnings, which opens the diagnostics.

In a small window the chart, the conserved regions and the position table do not all fit. The
column that holds them then scrolls: drag the scroll bar at its right edge, or turn the mouse
wheel over the conserved regions. Over the chart the wheel zooms the chart instead, and over the
table it scrolls the table's rows; it scrolls the column there only once the rows can scroll no
further. The table always keeps at least four rows.

Values are rounded only for display; sorting and exports keep every digit. A value that is not
zero never reads as zero: when it is smaller than the smallest step the display shows, it reads
as less than that step, for example "< 0.0001" for an entropy or "< 0.01%" for an incidence.
Likewise, a coverage or incidence just below its maximum never reads as the maximum, for example
"> 99.99%" for an incidence. Some places show fewer decimals than others, so the same coverage
can read "> 0.9999" among the facts of the selected position and "> 0.999" in the table.

**Entropy chart.** One line along the alignment, encoded so that the meaning never depends on
colour alone:

- **solid blue**: positions with support above T, whose entropy the paper's estimator corrects;
- **dashed vermillion**: positions with support at or below T (LS, ELS), which the paper's
  estimator reports uncorrected (the observed entropy). With another estimator every position is
  estimated the same way, and the legend says "eligible" and "low support" instead;
- **hatched bands**: positions without usable support, which have no entropy — never drawn as 0.
  A position that has support but no value (possible only in a results file edited by hand)
  breaks the line instead;
- a horizontal line at the mean entropy of eligible positions.

The value axis runs from 0 to 5% above the largest value shown, and is at least 0.005 bits tall
(1 bit when every value shown is 0).

Scroll to zoom about the pointer, drag to pan, Shift-drag to zoom to a region, double-click to
show everything; hovering a point shows its position, entropy, support and class, and clicking
selects it. Long alignments are drawn with M4 downsampling, which keeps every peak and trough.

**Plot similarity-weighted entropy.** When the run computed
[similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) (see
[Similarity-weighted entropy](#similarity-weighted-entropy) above), a **Metric:** row above the
chart offers **Entropy** and **Similarity-weighted entropy**; without it there is no such row. To
switch:

1. Click **Similarity-weighted entropy** in the **Metric:** row.
2. The chart redraws the same positions (more if you had narrowed the value range) with
   similarity-weighted entropy. Its heading and value axis read *Similarity-weighted entropy* and
   *Similarity-weighted entropy (bits)*, and the axis follows the largest similarity-weighted
   value shown (the same rule as above). The dashed horizontal line is now the mean
   similarity-weighted entropy of eligible positions. The legend reads *support > T* for the
   solid blue line and *support ≤ T, not in the summary* for the dashed vermillion line: no
   similarity-weighted value is corrected, and positions with support at or below T are left out
   of its summary.
3. The zoom and the selected position stay as they were. The value range of the filter bar now
   applies to similarity-weighted entropy and starts at its full range. If you had narrowed the
   entropy range, a note says *Value range reset to the full range of similarity-weighted
   entropy*, and the position table scrolls back to the selected row, which the positions shown
   again may have pushed out of view.

Click **Entropy** to go back (the note then names entropy). From the keyboard, press Tab until a
button of the **Metric:** row has the focus (Tab / Shift+Tab: move between controls) and press
Space to select it (Space: press or select). On the way, Tab passes through the filter bar's
fields; passing through a field without typing in it keeps the range it shows. While a button,
check box or radio button has the focus, the arrow keys move between controls instead of along
the position table. In a value field, Up and Down change the value and Left and Right move the
text cursor; in a text field, the arrow keys move the text cursor. Press Esc to give the arrow
keys back to the table. The command palette (below) offers the switch too. Every dataset you
open or analyse starts with entropy. Nothing you can click moves when you switch, so you can
switch back and forth in one place to compare the two lines: the filter bar's two value fields,
for example, keep their width for either metric. Each metric has its own value axis, so compare
their shapes, not their heights: similarity-weighted entropy is never more than the observed
entropy and is often much smaller.

**Conserved regions** appear as a strip under the chart, aligned with it at every zoom. The
threshold and the "eligible positions only" switch recompute them at once; flagged regions
(resting on positions with support ≤ T) are hatched and marked with an asterisk, and their
tooltip shows the minimum incidence, minimum support and 95% lower bound.

**Positions and variants.** The position table lists every shown position and can be sorted by
any column; the panels on the right describe the selected position (class, entropies and the
method that produced them, support, incidences, coverage), its motif composition, its variants
(searchable, filterable by incidence) and, for the selected variant, its counts per header value.
The filter bar restricts the view by position range, entropy range (the range of
similarity-weighted entropy while the chart shows it), motif class and support class. To change a
bound of the value range, drag its field right or up to raise it, left or down to lower it, or
click the field and type a value. Each pixel of a drag moves the bound by 1/200 of the metric's
full range, so 200 pixels cross the whole range; a range below 0.02 bits moves by 0.0001 per
pixel instead, and takes fewer. The fields show four decimals; a minimum that reads 0.0000 is 0,
and a maximum that reads as the largest value is that value exactly, so the full range hides no
position.

When the table's columns do not fit the window, the table scrolls sideways: drag its horizontal
scroll bar, scroll sideways on a touchpad, or hold Shift and turn the mouse wheel over the table.
The table's own vertical scroll bar is then at its right edge, out of view until you scroll there;
the mouse wheel scrolls the rows anyway. Moving through the rows with the arrow keys keeps the
sideways position, and a column header reached with Tab scrolls into view. Column widths you set
by dragging a header's edge are kept until you close the app, separately for each set of
columns. The set depends on the run: the **Similarity-weighted** column appears only when it
computed similarity-weighted entropy, and the **Coverage** column only when it set a minimum
sample coverage. Results with another set of columns show the widths you set for that set, or
its default widths the first time.

**Similarity-weighted values.** When the run computed similarity-weighted entropy, the Results
screen also shows:

- a chip right after the *mean* chip, **similarity-weighted mean X bits (observed Y)**: the mean
  similarity-weighted entropy of the eligible positions and, in parentheses, the mean observed
  (uncorrected) entropy of the same positions, so the two are compared like for like. The *mean*
  chip before it is the run's entropy, estimated with the run's estimator (the paper's correction
  by default). For the bundled MERS spike sample (`samples/mers_spike_aa.fasta`, default options
  with **Compute similarity-weighted entropy (Wu 2025)** ticked) the chip reads
  *similarity-weighted mean 0.0363 bits (observed 0.1207)*, next to *mean 0.1261 bits*: the
  similarity-weighted mean is about 30% of the observed mean of the same positions. With no
  eligible position it reads *similarity-weighted mean —*;
- a **Similarity-weighted** column in the position table, right after **Entropy** and sortable
  like the others (its tooltip says *Sort by similarity-weighted entropy*). In the exported
  positions table the column stays at the end, as `dima analyze` writes it;
- three more facts of the selected position: **Similarity-weighted entropy** (in bits),
  **Normalised observed entropy** and **Normalised similarity-weighted entropy**. A normalised
  value is the entropy divided by log₂ of the number of distinct k-mers at the position (the
  [Index](/reference/glossary/#index-major-minor-and-unique) included), so it lies between
  0 and 1. For MERS spike position 25, the position of the methodology page's
  [worked example](/explanation/entropy/#worked-example), they read *0.4275 bits*, *0.4282* and
  *0.1287*;
- the same three values in the tooltip of a point of the chart.

Read them with care:

- With a single distinct k-mer the normalised values are undefined and read *— (one variant)*,
  while similarity-weighted entropy reads *0.0000 bits*. Without support all three read *—*.
- At positions with support at or below T (LS, ELS) the two normalised values carry the mark
  *(low support)*. They come from few sequences: they are less reliable and can be higher or
  lower than they would be with more sequences. In the HIV-1 pol protein data set
  (`tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2`), position 3 has support 55 and 17 distinct
  9-mers; its facts read *0.8301 (low support)* and *0.2503 (low support)*.
- Where every sequence carries a different k-mer, normalised observed entropy is 1.0000 by
  arithmetic alone, whatever the sequences: in the same data set, position 29 has 4 sequences
  with 4 different 9-mers and reads *1.0000 (low support)*.

**Command palette.** Ctrl+K (⌘K on macOS) jumps to a position, finds the positions that carry a
k-mer, or runs a command. On the Results screen of a run with similarity-weighted entropy it also
offers **Show similarity-weighted entropy** or **Show entropy**, whichever the chart does not
show. **Help ▸ Keyboard shortcuts** lists the other keys, among them Tab / Shift+Tab: move between
controls; Space: press or select; and Esc, which closes the palette and also gives the arrow keys
back to the table.

## Export

The export menu writes:

| Export | Same as |
|---|---|
| Results (JSON) — `.json`, `.json.gz` or `.json.zst` by name | `dima analyze -o` |
| Positions (JSON Lines) | `dima analyze -o x.jsonl` |
| Tables (TSV folder) | `dima analyze --tsv-dir` |
| Conserved regions (JSON or TSV), at the current threshold | `dima hcs` |
| Entropy chart (PNG or SVG), as shown — or Similarity-weighted entropy chart, when the chart shows it | — |
| Run report | `dima analyze --run-report` |
| Methods & references, References (BibTeX) | `dima cite --from` with the results and the conserved regions |

The entropy chart is exported at the size set in **Preferences**, with its text at the same
size in the PNG and the SVG. Its legend continues on further rows when it is wider than the
figure, and a title, subtitle or legend label too long for the figure is shortened with "…"
(the SVG keeps the whole title as its document title).

The chart is exported as it is shown, so with the chart switched to similarity-weighted entropy:

- the menu items read **Similarity-weighted entropy chart (PNG)…** and **(SVG)…**;
- the figure's title reads *<query> — Similarity-weighted entropy per k-mer position*, its value
  axis *Similarity-weighted entropy (bits)*, and its legend and dashed line are those of the
  chart; the subtitle leaves out the estimator, which applies only to entropy;
- the save dialog proposes `<query>.similarity-weighted-entropy.png` (or `.svg`) instead of
  `<query>.entropy.png`, so the figures of both metrics can be kept side by side.

A figure is saved for the metric the chart showed when you chose the export. If the chart is
switched while the save dialog is open (with the command palette, for instance), nothing is
saved and a warning says *Nothing was saved: the chart was switched to similarity-weighted
entropy while the dialog was open. Export again to save the chart now shown.* (or *…switched to
entropy…*). Choose the export again: the dialog then proposes the name of the chart now shown.
The other exports do not depend on the chart and are saved as usual.

Every file is written completely or not at all. **Run details** shows how the analysis ran
(equivalent command, threads, phase times, platform, whether similarity-weighted entropy was
computed), and **Help ▸ Cite DiMA** gives the citation
of the paper and of this version, with **Copy citation** and **Copy BibTeX**.
