---
title: Use the desktop app
description: Analyse an alignment in the DiMA desktop app — a first analysis step by step, then every screen and control, the exports, where the app keeps its settings, and what screen readers hear.
---

The desktop app, `dima-gui`, runs the same library as the command line: the same defaults,
validation rules, results and file formats. A results file written by `dima analyze` opens in the
app, and the app's exports read back in `dima view`, `dima hcs` and `dima cite`. It has two
screens, **Setup** (choose a file and the options, run) and **Results** (explore what the run
found), switched with the buttons at the left of the top bar.

The terms used on this page are defined in the [Glossary](/reference/glossary/). The most
important ones: a [k-mer](/reference/glossary/#k-mer-and-k) is a window of k residues of one
sequence; a [position](/reference/glossary/#position) is the place where a k-mer window starts;
a position's [support](/reference/glossary/#support) is the number of sequences with a
[valid k-mer](/reference/glossary/#valid-k-mer) there; and positions supported by more than the
[support threshold](/reference/glossary/#support-threshold) T are
[eligible](/reference/glossary/#eligible-position).

## First analysis in the app

These steps analyse the alignment behind Fig. 1C of the DiMA paper [@tharanga2025]: 191
MERS-CoV spike protein sequences. They take a few minutes.

1. **Install the app.** Download the package for your system from the release page and install
   it as [Install DiMA](/getting-started/install/#desktop-app) describes. On Windows, run
   `DiMA-1.2.0-x86_64.msi`; if Microsoft Defender SmartScreen says "Windows protected your PC",
   choose **More info**, check that the app is DiMA, then **Run anyway**. On macOS, open the
   `.dmg` and drag DiMA to Applications; the install page gives the steps for the first launch.
2. **Start DiMA**: from the Start menu (Windows), the Applications folder (macOS) or your
   desktop's app menu (the Linux `.deb`). The Linux AppImage and `.tar.gz` start from the file
   itself (`./DiMA-1.2.0-x86_64.AppImage`, or `./dima-gui` in the unpacked folder). The app opens
   on the **Setup** screen. Starting it as `dima-gui FILE` opens nothing: the app ignores
   arguments, so open the file from the app (step 4).
3. **Get the sample alignment.** Download
   [`mers_spike_aa.fasta`](https://raw.githubusercontent.com/ShanWeera/dima-perf/master/samples/mers_spike_aa.fasta)
   (`samples/mers_spike_aa.fasta` in the repository; the same file as in step 1 of
   [Your first analysis](/getting-started/first-analysis/#1-get-the-alignment)). The release
   packages do not contain it. If your browser shows the file instead of saving it, use its
   **Save Page As** command; it may then save it as `mers_spike_aa.fasta.txt`, which the app
   opens all the same (step 4 says how to find it).
4. **Open it.** Press **Open…**, at the top of the **Input** card, and choose the file. The file
   dialog lists alignments and results files first; if you do not see a file saved as `.txt`,
   choose **All files** in the dialog's file-type list (on macOS the dialog lists every file).
   You can also drop the file on the window. Within a moment the card shows what the app found
   in the file:

   | Fact | Value |
   |---|---|
   | Sequences | 191 sequences |
   | Alignment length | 1353 columns |
   | Alphabet | Protein (detected from 236346 residues) |
   | Default k | 9 |
   | File | 277 kB, compression: none |
   | Headers | 4 fields '\|'-separated |

5. **Leave the options as they are.** They are the paper's: k = 9 and T = 100 for proteins.
6. **Press ▶ Analyze.** The button turns into a progress card for a moment, a notification
   says *Done: Analysed 191 sequences, 1345 positions in 0.01 s* (the time varies), and the
   **Results** screen opens by itself.
7. **Read the top of the Results screen.** The chips say *191 sequences*, *1345 positions*,
   *k = 9*, *T = 100 (support > T eligible)*, *mean 0.1261 bits*, *lowest 0.0000 at position 7,
   455 positions tied*, *highest 1.5878 at position 26*, *low support 0 (NS 0, LS 0, ELS 0)* and
   *total variants at 1: 1.17%*. The mean is the
   [eligible mean](/reference/glossary/#eligible-mean): the average entropy of the eligible
   positions, here all 1345 of them. Below the chips, the chart draws each position's
   [entropy](/reference/glossary/#entropy-reported-entropy) along the alignment, the strip under
   it shows the 44 conserved regions, and the table lists the positions.
8. **Look at one position.** Click position 25 in the chart or in the table (or press Ctrl+K,
   ⌘K on macOS, type `25` and press Enter). The panel on the right shows its facts: entropy
   1.4617 bits, observed entropy 1.4224 bits, support 170, and its ten variants, the most common
   (the Index) `SVKSACIEV` carried by 129 sequences.
9. **Export the chart.** Choose **Export** in the top bar, then **Entropy chart (PNG)…**. The
   save dialog proposes `mers_spike_aa.entropy.png`; press **Save**. A notification names the
   file saved.
10. **Export the methods.** **Export ▸ Methods & references…** saves
    `mers_spike_aa.methods.txt`: a methods paragraph describing this analysis and the numbered
    references it used, as `dima cite --from` prints them.
11. **Cite DiMA.** **Help ▸ Cite DiMA…** shows the citation of the paper and of this version of
    the software; **Copy citation** copies both.

The rest of this page describes every screen and control in the order you meet them.

## The Setup screen

The Setup screen is one column of cards: **Input**, **Analysis options**, **Header fields** (only
while variants are stratified by header fields), then **▶ Analyze** and **Recent files**.

### Open an alignment

The app opens an aligned FASTA file (plain, or compressed with gzip, bzip2 or Zstandard) or a
results file written by DiMA (`.json`, `.json.gz` or `.json.zst`). It tells them apart by their
content, not their name: an alignment shows on the Setup screen, a results file goes straight to
the Results screen. There are five ways to open a file:

- **Open…**, on the Input card or in the top bar: the file dialog (see below).
- Drop a file on the window. With several files, the app opens the first and says *One file at
  a time: opening NAME*. While no alignment is open, the Input card shows a drop zone: *Drop an
  alignment (.fasta, .gz, .bz2, .zst) or a results file here*.
- Type or paste a path into the field *or paste a path here and press Enter*, then press Enter or
  **Open**. The field takes the forms file managers copy: a path in quotes (Windows **Copy as
  path**), a `file://` address (Linux and macOS file managers) and a path starting with `~` for
  your home folder.
- Click a file in **Recent files**, at the bottom of the screen. The list keeps the last ten
  files, newest first, with what each was (*191 sequences, protein*, or *results, protein*) and
  when you opened it (*just now*, *5 min ago*, *yesterday*, *3 weeks ago*). **×** removes one
  file from the list and **Clear all** empties it. A file that no longer exists is reported when
  you click it and removed from the list.
- The command palette's **Open a file…** (see [The command palette](#the-command-palette)).

While the app looks at a file, the card shows *Checking NAME…* with a **Cancel** button.
Opening another file while an analysis runs first asks, in a dialog titled *An analysis is
running*: *Cancel it and open NAME? Nothing of the running analysis is kept.*, with **Cancel
analysis and open** and **Keep as is**.

**File dialogs.** The app uses your system's file dialog, titled *Open an alignment or a DiMA
results file*. While it waits for it, *Waiting for the system file dialog…* shows with **Use
built-in browser** (on the Input card on the Setup screen, in the top bar on the Results
screen): if the dialog does not appear, for example on a remote desktop, that button opens
DiMA's own file browser instead. When the system dialog closes at once without a choice, the
app says *The system file dialog is not available here; using the built-in browser* and opens
its own; in a remote session without a desktop it uses its own from the start. You can choose
the built-in browser for good in **Preferences**, which also sets the size of exported figures
(see [Theme, Preferences and About](#theme-preferences-and-about)).

**Results of a newer version.** A results file written by a newer version of DiMA (a later
[schema version](/reference/glossary/#schema-version)) opens with a warning, *The results were
written with schema 1.2.0; this version writes 1.1.0 and keeps only the fields it knows. Use the
DiMA version that wrote the file to keep every field.* (a version that is not three plain numbers
appears in quotes). You can explore the results as usual, but an export from this version leaves
out what only the newer version records; open the file in the version that wrote it to keep
everything.

**Files the app does not open.** The card *Error: NAME could not be opened* says why, with a
hint. Two cases have their own messages:

- *'mers.json.bz2' is compressed with bzip2, which DiMA does not read results from* — DiMA reads
  results only plain or compressed with gzip or Zstandard (alignments compressed with bzip2 open
  as they are). Decompress the file (`bunzip2 mers.json.bz2`), or recompress it with gzip or
  Zstandard, and open it again.
- *'results.json' is compressed with xz (.xz), which DiMA does not read* (likewise Unix compress
  (.Z), LZ4 and legacy LZMA (.lzma)), or *'download.zip' is a zip archive, which DiMA does not
  read* (likewise a 7z archive): DiMA reads neither alignments nor results in these formats,
  whatever the file's name. Decompress the file, or recompress it with gzip or Zstandard; extract
  an archive first.

Other problems with an alignment are those of the command line; see
[Troubleshooting](/how-to/troubleshooting/).

### The pre-check

Before you run anything, the app reads the beginning of an alignment, the first batch of
records (up to 10,000 records or 64 MiB), in well under a second even for a large compressed
file. The Input card then shows the file's name (hover over it for its full path) and these
facts:

- **Sequences**: the exact count when the whole file fit in the first batch (*191 sequences*);
  otherwise an estimate from the bytes read per record, *~N sequences (estimated)*, or, when the
  size is unknown, *more than N sequences*. A note then says *Checked the first 10000 records; the
  analysis reads the whole file and reports any problem with its exact location.*
- **Alignment length**: the columns of the first record (*1353 columns*).
- **Alphabet**: the alphabet detected from the residue letters of the first batch, *Protein
  (detected from 236346 residues)*. Auto detects it with the rules of Easel's
  `esl_abc_GuessAlphabet`, the sequence library of HMMER [@eddy2011], which make no call when the
  letters do not decide it (see [Alphabet](/reference/glossary/#alphabet)).
- **Default k**: the k-mer length the alphabet implies, 9 for proteins and 27 for DNA and RNA.
- **File**: its size and compression (*277 kB, compression: none*).
- **Headers**: how many `|`-separated fields the first header has (*4 fields '|'-separated*) and,
  when some of the checked headers have another number of fields, how many, for example *4
  fields '|'-separated; 3 of the first 191 headers differ*.

When the alphabet cannot be detected, the fact reads *undetermined — choose one below* and, while
**Alphabet** is **Auto**, a warning gives the reason, one of:

- *Alphabet not detected: only 3 residue letters in the first record; that is too few to tell
  protein from nucleotide sequences. Choose it under Alphabet below.* (ten letters or fewer);
- *Alphabet not detected: the sequences contain both T (2) and U (2), so DNA and RNA cannot be
  told apart. Choose it under Alphabet below.*;
- *Alphabet not detected: the residue composition of the first 10000 records (60000 letters)
  does not identify the alphabet. Choose it under Alphabet below.* This is what
  `tests/data/pstvd_loop27_ir1.fasta.bz2` in the repository gives: its first 10,000 records hold
  only A, C and T, and without a G the rules cannot call it DNA.

Choose **Protein**, **DNA** or **RNA** under **Alphabet**: the warning goes, and the fact reads
*undetermined (Protein chosen below)*.

You can start the analysis right away: the run itself reads and checks the whole file. With the
alphabet on **Auto**, the run detects it again exactly as `dima analyze` does, so the results and
their provenance match the command line. If the file changed after it was checked, the run stops
before counting and the app says *NAME changed after it was checked. It was checked again: review
the settings and press Analyze.*

### The analysis options

The **Analysis options** card holds the paper's options. Every option has a tooltip with its
meaning and its references; hover over its name or its control.

| Option | What it sets |
|---|---|
| **Alphabet** | **Auto** (it shows what it detected: *Auto (detected: Protein)*), **Protein**, **DNA** or **RNA**. T and U are folded to the alphabet's letter and lower case to upper case. Reset for every new file. |
| **k-mer length** | k. Next to it, *default for the alphabet*, or *set once the alphabet is known* while the alphabet is undetermined. Once you change it, **Use default** goes back to the alphabet's default. Any value from 1 to the alignment length. Reset for every new file. |
| **Support threshold T** | T (default 100, as in the paper): *positions with support > T are eligible*. Eligible positions enter the summaries. |
| **Query name** | The name of the protein or gene in titles and file names. Empty means the file name without its FASTA and compression extensions; the field shows that name greyed out. Reset for every new file. |
| **Conserved regions (HCS) at** | The [HCS threshold](/reference/glossary/#hcs-threshold): the [Index incidence](/reference/glossary/#incidence) a position needs to be part of a [conserved region](/reference/glossary/#historically-conserved-sequence-hcs). The slider moves from 50.5 to 100 in steps of 0.5; the field after **Index incidence** takes an exact value such as 95.25 (above 50, at most 100, at most 9 decimals). An invalid value is shown in red and the last valid value is kept. Default 100%, the paper's. |
| **Eligible positions only** | Builds conserved regions from eligible positions only; see [Eligible positions only](/reference/glossary/#eligible-positions-only). |
| **Stratify variants by FASTA header fields** | Counts every variant per value of the header fields you choose (see [Describe the header fields](#describe-the-header-fields)). Off by default, as on the command line. |

With the paper's estimator, an eligible position's entropy is corrected *where possible*
[@tharanga2025, Fig. 2]. The correction needs at least five regression points (the full data set
included; a minimum taken from DiMA v5.0.9, which the paper does not state), so a position is
corrected only when its support is at least 1.25 T (125 sequences for T = 100). Between T and
that, and in the one tiny case T = 2, N = 3 (N being the support), it keeps its
[observed entropy](/reference/glossary/#observed-entropy). Such positions are still eligible and
in the summaries. To have every position supported by all your sequences corrected, choose T no
higher than 0.8 times the number of sequences.

The conserved-region options apply to the results shown as soon as you change them (see
[The conserved-regions strip](#the-conserved-regions-strip)); the others apply to the next run.

**Advanced.** Click **Advanced** to open the section; the app remembers whether you left it open,
also after a restart. Its title counts the options inside it that differ from their defaults, for
example **Advanced (2 changed)**, so a changed option is seen while it is closed.

| Option | What it sets |
|---|---|
| **Entropy estimator** | [The estimator](/reference/glossary/#estimator): **DiMA (paper, Fig. 2)** (the paper's correction, `dima` on the command line), **Chao–Wang–Jost** (`chao-wang-jost`), **Miller–Madow** (`miller-madow`) or **Raw (plug-in)** (`raw`, the observed entropy everywhere). Chao–Wang–Jost and Miller–Madow estimate every position with at least 2 sequences, eligible or not; a position with one sequence keeps its observed entropy (0 bits). |
| **Validation** | **Strict** (default) stops at the first character outside the alphabet, with its record, line and column; **Permissive** excludes the k-mers that contain such characters and reports one warning; **Report** runs as Permissive in the app (on the command line it also prints the table of every character, which the app's [diagnostics window](#the-diagnostics-window) shows after every run). See [Validation mode](/reference/glossary/#validation-mode). |
| **Fill value** | The value given to empty header fields and, in permissive mode, to every stratified field of a header with the wrong number of fields. Default *Unknown*. Used, and editable, only while variants are stratified; empty, it reads *must not be empty* in red. |
| **Minimum coverage** | Tick **flag below** to flag positions whose [sample coverage](/reference/glossary/#sample-coverage) is below a value (0.95 when you tick it; from 0 to 1, three decimals). See [Minimum coverage](/reference/glossary/#minimum-coverage) and below. |
| **Similarity-weighted** | **Compute similarity-weighted entropy (Wu 2025)**; see [Similarity-weighted entropy](#similarity-weighted-entropy). Off by default. |
| **Worker threads** | The threads the analysis uses, *N of M CPUs* after the slider. Default: all CPUs but one (at least one), so the app stays responsive; the threads run at reduced priority. Results do not depend on the number of threads. |

**Minimum coverage** changes no value: it adds a flag. With it, the run marks each position below
the value with a purple tick 7 pixels high at the top of the chart, adds the legend entry *sample
coverage < 0.98* (also in exported figures), a **Coverage** column in the position table with
**▼** at those positions (its tooltip: *Below the minimum coverage of this run*), the words
*(below the minimum)* after the coverage in the position's facts, a line *sample coverage 0.977
below the minimum* in the chart's tooltip, and the warning `below_min_coverage`. On the MERS
sample, whose lowest coverage is 0.9646, 0.95 flags nothing and 0.98 flags 42 positions (21–26,
193–198, …): *42 positions have sample coverage below 0.98*. A coverage just below the minimum
never reads as the minimum: 0.94996 below 0.95 reads *> 0.9499*, not *0.9500*.

#### Similarity-weighted entropy

[Similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) is an
extra measure of diversity that is not part of the DiMA method, so it is off by default. It
counts two variants that differ at one residue as less diverse than two unrelated variants;
Shannon entropy counts them alike. To compute it:

1. Click **Advanced** to open the section.
2. Tick **Compute similarity-weighted entropy (Wu 2025)**, in the row named
   **Similarity-weighted**. A dot appears after the row's name (its tooltip: *Changed from the
   default (default: off)*), and the count in the section's title goes up by one, also when the
   section is closed: **Advanced (1 changed)** if no other option in it was changed.
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

#### Changed options and Reset to defaults

An option that differs from its default is marked with a dot after its name, whose tooltip names
the default, for example *Changed from the default (default: 100)*. The dot's place is kept free
when there is no dot, so no control moves when you change an option: a second click at the same
place reaches the same control. **Reset to defaults** (greyed out while every option is at its
default) restores the analysis options, the conserved-region threshold and switch, and the worker
threads. It does not change the header fields, **Preferences**, the recent files, or the window,
theme and interface zoom.

The app remembers the options that do not depend on the file between sessions: T, the
estimator, the validation mode, the minimum coverage, the fill value, similarity-weighted
entropy, the conserved-region threshold and switch, and the worker threads (see
[Where the app keeps its settings](#where-the-app-keeps-its-settings)). The alphabet, k and the
query name belong to one file and start again from their defaults with every new file.

### Describe the header fields

If the headers hold `|`-separated fields, tick **Stratify variants by FASTA header fields** to
count every variant per value of the fields you choose (the
[stratified fields](/reference/glossary/#stratified-field)): per host or per year, for example.
The **Header fields** card then opens, with one row per field of the first header:

- **#** and **Field**: the field's name, one of the paper's names (**Accession**, **Strain**,
  **Species**, **Year**, **Geolocation**, **Host**) or **Custom…**, which shows a name field.
  Rows start as `field_1`, `field_2`, …, never as values from the file.
- **Stratify**: whether variants are counted per value of this field. Every field is ticked at
  first, as on the command line.
- **Sample values**: up to three values from the checked records (hover over one for all of it).
  A field whose values are all different in at least 100 checked records is marked
  *identifier-like* (its tooltip: *All values in the checked records are different*): counting
  by an accession multiplies the size of the results, so untick it (see
  [Identifier-like field](/reference/glossary/#identifier-like-field)).

**Paste format…** opens a field for a format such as `Accession|Host|Country|Year` (a whole
`--header-format` option, with or without quotes, works too); **Apply** or Enter names the rows.
If the number of names differs from the first header's fields, a note says so: *3 fields pasted,
but the first header has 4: headers that do not match are an error in strict mode*. **Copy as
--header-format** copies the equivalent command-line options, with `--metadata-fields` when only
some fields are ticked, for example `--header-format 'Accession|Host|Country|Year'
--metadata-fields 'Host|Country|Year'`; a note says *Header format copied*.

When some of the checked headers have another number of fields than the first, the card says so:
*3 of the checked headers have a different number of fields than the first.* In the default
strict mode the run then stops with the list of those headers. Correct the headers, or choose
**Permissive** under **Advanced ▸ Validation**: the stratified values of those headers then
become the fill value. This matters only while variants are stratified. See
[Stratify by header metadata](/how-to/metadata/) for how values are cleaned and what the
warnings about them mean.

A problem with the names shows in red under the rows, and keeps **Analyze** greyed out:

- *header format field 2 has no name*: give the field a name;
- *header format field "Host" appears more than once*: give every field a distinct name;
- *tick at least one Stratify box*: tick a box, or untick **Stratify variants by FASTA header
  fields**.

### Why Analyze is greyed out

**▶ Analyze** is greyed out while something would stop the run before it reads the file. The
reason shows in red under the button and as its tooltip:

| Reason | What to do |
|---|---|
| *Open an alignment first* | Open a file (the reason then shows only as the tooltip). |
| *Choose the alphabet: only 3 residue letters in the first record; that is too few to tell protein from nucleotide sequences* (or another reason, as in [The pre-check](#the-pre-check)) | Choose **Protein**, **DNA** or **RNA** under **Alphabet**. |
| *the default k for proteins (9) exceeds the alignment length (6 columns): choose a k-mer length of at most 6* (or *k = 12 exceeds …* for a k you set; *1 column* for an alignment one column wide) | Set **k-mer length** to at most the alignment length. A default k longer than the alignment is shown as it is, never replaced by the app. |
| *the Fill value under Advanced must not be empty* | Type a fill value under **Advanced**, or stop stratifying. |
| *header format field 2 has no name*, *… appears more than once*, *tick at least one Stratify box* | As in [Describe the header fields](#describe-the-header-fields). |

### Run the analysis

**▶ Analyze** starts the run. The button becomes a card with a spinner, the step and its
progress, a bar where the progress is known, and **Cancel**:

- *Starting…*, until the run reports;
- *Step 1 of 3: opening the input; 0.02 s*: opening the file, reading the first batch and
  detecting the alphabet;
- *Step 2 of 3: counting k-mers; 191 sequences read; 277 kB of 277 kB; 0.01 s*: the bytes read of
  the stored (compressed) file;
- *Step 3 of 3: computing statistics; 600 of 1345 positions; 0.01 s*;
- *Finishing; 0.02 s*.

**Cancel** shows *Cancelling…* until the run has stopped, and then *Analysis cancelled — no
results were written*. A run that finishes opens the Results screen and says *Done: Analysed 191
sequences, 1345 positions in 0.01 s*. If the system refused some of the worker threads, a warning
says *Used 1 of 4 worker threads: the system refused to start more. Results are the same with any
number of threads; to avoid this, close other programs or lower Worker threads under Advanced
(Setup screen).* While results are loaded, **Back to the results ▶** under **Analyze** returns
to them.

### When the analysis stops: the error card

If the alignment has a problem, the run stops and the Setup screen shows a red card, *Error: The
analysis stopped — no results were written*. It holds:

- the message, for example *malformed.fasta: record 5 (line 98, column 2, byte 5854): character
  '1' (0x31) is not valid in a protein alignment*;
- the place, row by row: **Record** (counted from 1), **Line**, **Column** (the alignment column,
  white space not counted) and **Byte** (counted from 0 in the decompressed text), as far as they
  are known;
- **Header**, the offending record's header line, and **Sequence**, the residues around the
  offending column with the column in brackets, up to 12 on each side: *M[1]IHSVFLLMFLLT…*
  (*reading the record…* while the app reads them back from the file; a character that cannot be
  shown is written as `\x00`);
- *Hint:* how to fix it. Hints name the command-line options; the table
  [In the desktop app](/reference/diagnostics/#in-the-desktop-app) gives the control for each;
- *No results were written.*

**Copy details** copies the card, with the version and the platform, for a report (*Error details
copied*). **Save run report…** saves the [run report](/reference/glossary/#run-report) of the
failed run, named after the alignment (`malformed.fasta.run-report.json`). **Dismiss** closes the
card. A file that cannot be opened gets the same card, titled *Error: NAME could not be opened*,
without the run's lines. See [Troubleshooting](/how-to/troubleshooting/) for the causes and fixes
of each message.

## Explore the results

The Results screen has the run's facts across the top, the filter bar below them, the chart, the
conserved regions and the position table in the middle, and the selected position on the right.

In a small window the chart, the conserved regions and the position table do not all fit. The
column that holds them then scrolls: drag the scroll bar at its right edge, or turn the mouse
wheel over the conserved regions. Over the chart the wheel zooms the chart instead, and over the
table it scrolls the table's rows; it scrolls the column there only once the rows can scroll no
further. The table always keeps at least four rows.

Values are rounded only for display; sorting and exports keep every digit. A value that is not
zero never reads as zero: when it is smaller than the smallest step the display shows, it reads
as less than that step, for example "< 0.0001" for an entropy or "< 0.01%" for an incidence.
Likewise, a coverage or incidence just below its maximum never reads as the maximum, for example
"> 99.99%" for an incidence, and a coverage below the run's minimum coverage never reads as that
minimum. Some places show fewer decimals than others, so the same coverage can read "> 0.9999"
among the facts of the selected position and "> 0.999" in the table. "—" means undefined (no
value exists), never zero.

### The top of the screen

The query name heads the screen; for results opened from a file, *opened from NAME* follows it
(hover over it for the full path). The chips under it, each with a tooltip that explains it (on
the MERS sample, default options):

| Chip | What it says |
|---|---|
| *191 sequences* | The sequences in the alignment. |
| *1345 positions* | The k-mer positions: the alignment length (1353) − k + 1. |
| *k = 9* | The k-mer length. |
| *T = 100 (support > T eligible)* | The support threshold; the tooltip says what the run's estimator did with the eligible positions (the paper's corrects them where possible, from support 125 for T = 100). |
| *mean 0.1261 bits* | The [eligible mean](/reference/glossary/#eligible-mean); *mean —* when no position is eligible. |
| *similarity-weighted mean 0.0363 bits (observed 0.1207)* | Only when the run computed similarity-weighted entropy; see [Similarity-weighted values](#similarity-weighted-values). |
| *lowest 0.0000 at position 7, 455 positions tied* | The lowest entropy among eligible positions, the first position with it and how many share it ([tied positions](/reference/glossary/#tied-positions)). |
| *highest 1.5878 at position 26* | The highest, alike. |
| *low support 0 (NS 0, LS 0, ELS 0)* | The [low-support count](/reference/glossary/#low-support-count): positions with support at or below T, by [class](/reference/glossary/#ns-ls-and-els). On the HIV-1 *pol* protein data set it reads *low support 713 (NS 551, LS 162, ELS 0)*. |
| *total variants at 1: 1.17%* | The [total variants incidence](/reference/glossary/#total-variants-incidence) of the selected position: the share of its supporting sequences that carry a variant other than the Index. The app selects the first position shown when results load. |

At the right of the top bar, the diagnostics badge reads *⚠ 1 warning* (or *⚠ N warnings*) when
the run or the conserved regions raised warnings, *N notes* when they raised only notes, and *No
warnings* otherwise; it opens [the diagnostics window](#the-diagnostics-window). **Run details**
opens [Run details](#run-details), and *Ctrl+K* (*⌘K* on macOS) is a reminder of the
[command palette](#the-command-palette).

### The entropy chart

One line along the alignment, the [entropy](/reference/glossary/#entropy-reported-entropy) of each
position from the run's estimator, encoded so that the meaning never depends on colour alone:

- **solid blue**: eligible positions (support above T). With the paper's estimator their entropy
  is corrected where possible (support of at least 1.25 T); an eligible position the correction
  does not reach keeps its observed entropy [@tharanga2025, Fig. 2];
- **dashed vermillion**: positions with support at or below T (LS, ELS), which the paper's
  estimator reports uncorrected (the observed entropy). With another estimator every position
  with at least 2 sequences is estimated alike, and the legend says "eligible" and "low support"
  instead;
- **hatched bands**, labelled *no usable support* where they are wide enough: positions without
  usable support (NS), which have no entropy — never drawn as 0. A position that has support but
  no value (possible only in a results file edited by hand) breaks the line instead;
- **dots**: a position whose neighbours have no point (they are hidden by the filters or have no
  entropy) is drawn as a dot, so it shows even without a line;
- a thin dashed grey horizontal line labelled **eligible mean** and its value, the average
  entropy of the eligible positions (see [Read the eligible mean line](#read-the-eligible-mean-line));
- a vertical line at the selected position, and short purple ticks at the top at positions below
  the run's minimum coverage, when it set one.

The legend above the chart names what it shows, with the run's T:

| Legend entry | When |
|---|---|
| *support > 100: corrected where possible* and *support ≤ 100: uncorrected* | Entropy, paper's estimator |
| *support > 100: eligible* and *support ≤ 100: low support* | Entropy, other estimators |
| *support > 100* and *support ≤ 100, not in the summary* | Similarity-weighted entropy |
| *NS: no usable support* | When the chart shows NS bands |
| *eligible mean* | When there is an eligible mean |
| *sample coverage < 0.98* | When the run set a minimum coverage and positions are below it |

Hover over a legend entry for the encoding explained, with its references; screen readers read the
same text. The value axis, *Entropy (bits)*, runs from 0 to 5% above the largest value shown, and
is at least 0.005 bits tall (1 bit when every value shown is 0); the position axis is labelled
*Position*. Long alignments are drawn with M4 downsampling, which keeps every peak and trough.

**Zoom and select.** Scroll over the chart to zoom about the pointer, drag to pan, Shift-drag to
zoom to a region (the hint *drag: pan • Shift-drag: zoom to region* says so), and double-click to
show everything. **+** zooms in, **−** zooms out and **Fit** shows every position shown; **Fit**
and **−** are greyed out until you zoom in. Click a point to select its position. When the
filters hide every position, the chart says *No positions match the current filters*.

**Tooltips.** Hovering a point marks it and its row in the table (the position in italics), and
shows, for MERS spike position 25:

```text
Position 25
entropy 1.4617 bits, observed 1.4224
method: paper's correction (dima)
support 170
eligible (support > 100): corrected entropy
```

The second line is the position's entropy and its observed entropy; the third, the
[entropy method](/reference/glossary/#entropy-method) in words and as the results file writes it,
with the [fallback](/reference/glossary/#entropy-fallback) when there is one (the
[position facts](#position-facts) list them all); the last, the class line: what the position's
class means for its value. On the HIV-1 *pol* protein data set
(`tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2`), position 42 has support 105, below the 125 the
correction needs, so it reads *method: observed entropy (plug_in; fallback: too few points)* and
*eligible (support > 100): observed entropy (the correction did not apply)*. With a minimum
coverage, a line such as *sample coverage 0.977 below the minimum* follows; with
similarity-weighted entropy, its three values. Hovering a band shows the positions it covers,
*Positions 30–41: no usable support (support 0, no entropy)* for the first band of the HIV-1
*pol* protein data set.

#### Read the eligible mean line

The dashed grey line across the chart is the
[eligible mean](/reference/glossary/#eligible-mean): the average entropy of the run's eligible
positions, the positions with support above T. Each eligible position counts once, whatever its
support. The line's value is written at its right end, and the legend lists it as *eligible
mean*; hover over that legend entry for a short explanation, which screen readers read too, as
for every legend entry. For the MERS spike sample (`samples/mers_spike_aa.fasta`, default
options) the line reads *eligible mean 0.1261*, the same number as the *mean 0.1261 bits* chip
above the chart and `average_entropy` in the exported results.

Use it as a reference level: a point above the line is a position with more entropy than the
average eligible position, a point below it one with less. Keep in mind:

- **It is the run's value, not that of the positions shown.** Zooming and panning do not move
  it, and the filters do not change its value. The value axis follows the largest value shown,
  though, so a filter can rescale the axis and move the line with it. If the filters hide the
  larger values, the line can lie above the top of the axis; it is then not drawn, but the
  legend still lists it.
- **Only eligible positions are in it.** The dashed vermillion stretches (support at or below T)
  are drawn but left out, and positions without support (hatched) have no entropy to add. In the
  HIV-1 *pol* protein data set (`tests/data/HIV1_ALL_2020_pol_PRO.fasta.bz2`, default options),
  476 of the 1,189 positions are eligible and the line reads *eligible mean 1.9072*; the 162
  positions drawn dashed, whose mean entropy is 0.5823 bits, are not in it. The mean of every
  point the chart draws there would be 1.5708 bits.
- **It does not show where the diversity is.** On the MERS spike sample 455 of the 1,345
  positions have entropy 0 and the highest, at position 26, has 1.5878 bits; the *lowest* and
  *highest* chips and the chart itself show that.
- **It depends on the run's settings.** It uses the run's estimator and T: on the MERS spike
  sample it is 0.1207 bits with the **Entropy estimator** *Raw (plug-in)* and 0.1235 bits with
  **Support threshold T** set to 30. Before you compare it between alignments, see
  [Can I compare diversity between proteins or datasets?](/getting-started/faq/#can-i-compare-diversity-between-proteins-or-datasets)

When the chart shows similarity-weighted entropy, the line is the eligible mean of
similarity-weighted entropy instead (see **Plot similarity-weighted entropy** below). An exported
figure of the chart has the same line, value and legend entry. When no position is eligible
there is no eligible mean: the *mean* chip reads *mean —*, and the chart has no such line and no
legend entry for it.

#### Plot similarity-weighted entropy

When the run computed
[similarity-weighted entropy](/explanation/entropy/#similarity-weighted-entropy-opt-in) (see
[Similarity-weighted entropy](#similarity-weighted-entropy) above), a **Metric:** row above the
chart offers **Entropy** and **Similarity-weighted entropy**; without it there is no such row. To
switch:

1. Click **Similarity-weighted entropy** in the **Metric:** row.
2. The chart redraws the same positions (more if you had narrowed the value range) with
   similarity-weighted entropy. Its heading and value axis read *Similarity-weighted entropy* and
   *Similarity-weighted entropy (bits)*, and the axis follows the largest similarity-weighted
   value shown (the same rule as above). The dashed grey line, still labelled *eligible mean*,
   is now the average similarity-weighted entropy of the eligible positions, the first number of
   the similarity-weighted chip: *eligible mean 0.0363* on the MERS spike sample. The legend
   reads *support > T* for the solid blue line and *support ≤ T, not in the summary* for the
   dashed vermillion line: no similarity-weighted value is corrected, and positions with support
   at or below T are left out of its summary.
3. The zoom and the selected position stay as they were. The value range of the filter bar now
   applies to similarity-weighted entropy and starts at its full range. If you had narrowed the
   entropy range, a note says *Value range reset to the full range of similarity-weighted
   entropy*, and the position table scrolls back to the selected row, which the positions shown
   again may have pushed out of view.

Click **Entropy** to go back (the note then names entropy). From the keyboard, the shortest way
is the command palette (below): press Ctrl+K (⌘K on macOS), type *similarity* and press Enter to
run **Show similarity-weighted entropy** (or type *entropy* and run **Show entropy** to go back).
With Tab instead, press Tab until a button of the **Metric:** row has the focus (Tab /
Shift+Tab: move between controls) and press Space to select it (Space: press or select). Tab
moves through the screen in this order: the top bar, the filter bar, the panel on the right (the
variant search, the minimum incidence, the variant table's column headers and the variants it
shows), then the **Metric:** row, the zoom buttons, the chart, the conserved regions and the
position table's column headers; that is about 30 presses from the top to the **Metric:** row.
Every stop is outlined while it has the focus, except the handles between the columns of each
table (they resize the columns), which egui's tables make keyboard stops. Passing through a
field of the filter bar without typing in it keeps the range it shows. While a button, check box
or radio button has the focus, the arrow keys move between controls instead of along the position
table. In a value field, Up and Down change the value and Left and Right move the text cursor; in
a text field, the arrow keys move the text cursor. Press Esc to give the arrow keys back to the
table. Every dataset you open or analyse starts with entropy. Nothing you can click moves when you
switch, so you can switch back and forth in one place to compare the two lines: the filter bar's
two value fields, for example, keep their width for either metric. Each metric has its own value
axis, so compare their shapes, not their heights: similarity-weighted entropy is never more than
the observed entropy and is often much smaller.

### The conserved-regions strip

Under the chart, **Conserved regions** gives their number, *44 regions, 0 flagged* on the MERS
sample at 100% (*1 region, 1 flagged* on the HIV-1 *pol* protein data set; *computing…* while
they are computed), then the conserved-region controls of the Setup screen, then the strip. The
regions are computed from the results shown, again whenever you change the threshold or
**Eligible positions only**, a quarter of a second after the last change. A
[flagged region](/reference/glossary/#flagged-region) rests on positions with support at or below
T; it is hatched and marked with an asterisk.

The strip lines up with the chart at every zoom. A region's bar covers the **alignment columns**
its k-mers cover, from the first column of its first member position to the last column of its
last member k-mer, so a bar runs k − 1 columns past its last conserved position and can lie under
positions whose own k-mers vary. On the MERS sample, region 1 is made of positions 7–14 and covers
columns 7–22, so its bar runs on under positions 15–22, whose entropy reaches 1.2990 bits. Read a
region's positions from its tooltip or the exported regions, not from the bar's extent.

Hovering a bar shows the region, for MERS region 1:

```text
Region 1: columns 7–22 (16 columns)
LLMFLLTPTESYVDVG
minimum Index incidence 100.00%
minimum support 170
lowest per-position 95% lower bound 97.9% (Clopper–Pearson)
```

The minimum incidence, the minimum support and the
[lower bound](/reference/glossary/#conservation-lower-bound) are the lowest among the region's
member positions, position by position; the bound is not the share of sequences that carry the
whole region. A flagged region adds its positions with support at or below T, the first twelve:
region 10 of the MERS spike nucleotide alignment (`tests/data/MERS_Spike_nt.fasta.bz2`) adds
*\* flagged: 29 positions with support ≤ 100: 3518, 3519, 3520, 3521, 3522, 3523, 3524, 3525,
3526, 3527, 3528, 3529 and 17 more*. A region that shares columns with the one before it adds
*split from the previous region (their overlapping k-mers disagree)*. See
[Find conserved sequences](/how-to/conserved-sequences/) and
[Conserved sequences](/explanation/conserved-sequences/).

Clicking a bar selects the region's first position. When no position reaches the threshold, the
strip says *No conserved regions at 100% — try a lower threshold*.

### The position table

The table lists every position the filters show:

| Column | What it shows |
|---|---|
| **Position** | The position (in italics while the chart hovers it). |
| **Entropy** | The reported [entropy](/reference/glossary/#entropy-reported-entropy), four decimals; *—* at NS positions. |
| **Similarity-weighted** | Only when the run computed it, right after **Entropy**. |
| **Support** | The [support](/reference/glossary/#support). |
| **Class** | ✔ for eligible positions (its tooltip: *eligible: support > 100*); *LS*, *ELS* or *NS* in the warning colour, whose tooltip is the class line, for example *LS, low support (support < 100): uncorrected entropy*. See [Support class](/reference/glossary/#support-class). |
| **Variants %** | The [total variants incidence](/reference/glossary/#total-variants-incidence), one decimal: *24.1%* at MERS position 25 (41 of its 170 sequences carry a variant other than the Index); *0.0%* at a conserved position; *100.0%* where no k-mer occurs twice; *—* without support. |
| **Coverage** | Only when the run set a minimum coverage: the sample coverage, three decimals, with ▼ below the minimum. Not sortable. |
| **Top variant** | The [top variant](/reference/glossary/#top-variant) and its incidence, *SVKSACIEV  75.9%* at MERS position 25: the first variant by count, ties in byte order. It is the Index, or the first of tied Index variants; where no k-mer occurs twice it is a Unique variant (HIV-1 *pol* protein position 29, support 4); empty without support. |

**Sorting.** Click a column's header, or press Space or Enter while it has the focus (it is then
outlined), to sort by it, smallest first (⏶); click again for largest first (⏷). Undefined values
(*—*) stay last in both directions. **Class** sorts ✔, ELS, LS, NS; **Top variant** sorts by the
k-mer, alphabetically, not by its percentage. Ties keep position order.

**Select and copy.** Click anywhere in a row, its text included, to select its position, or move
along the rows with the arrow keys, Home and End (they follow the sorted order). Right-click a row
for **Copy row**: its cells as the table shows them, separated by tabs, so they paste into a
spreadsheet as one row. The text in the tables cannot be selected, so that a click anywhere
selects the row; the exports hold every value with all its digits. When the filters hide every
position, the table says *No positions match the current filters — Reset shows all*; results
without positions say *These results contain no positions*.

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

### The panel on the right

The panel on the right describes the selected position: its facts, its motif composition, its
variants and, for the selected variant, its metadata. Before a position is selected it says
*Select a position in the chart or the table*.

#### Position facts

Under the heading *Position 25*, the class line says what the position's
[class](/reference/glossary/#support-class) means for its entropy (green when eligible, in the
warning colour otherwise), then the facts follow:

| Fact | MERS position 25 | What it is |
|---|---|---|
| **Entropy** | 1.4617 bits | The reported entropy, from the run's estimator. |
| **Method** | paper's correction (dima) | How the entropy was computed, in words, then as the results file writes it. |
| **Observed entropy** | 1.4224 bits | The [observed](/reference/glossary/#observed-entropy) (plug-in) entropy. |
| **Support** | 170 | The sequences with a valid k-mer here. |
| **Distinct variants** | 9 (21.95%) | The number of variants other than the Index, and the [distinct variants incidence](/reference/glossary/#distinct-variants-incidence): that number per 100 sequences that carry one of them, not per 100 supporting sequences (9 variants among the 41 sequences without the Index: 9 / 41 = 21.95%). *0 (—)* where every sequence carries the Index. Not the [distinct k-mers](/reference/glossary/#distinct-k-mers-d) D, which count the Index too (D = 10 here). |
| **Total variants** | 24.12% | The [total variants incidence](/reference/glossary/#total-variants-incidence). |
| **Sample coverage** | 0.9766 | The [sample coverage](/reference/glossary/#sample-coverage); *(below the minimum)* follows it at a position below the run's minimum coverage. |

The **Method** fact takes these values:

| Method | Means |
|---|---|
| *paper's correction (dima)* | The paper's correction [@tharanga2025, Fig. 2]. |
| *Chao–Wang–Jost (chao_wang_jost)*, *Miller–Madow (miller_madow)* | The other estimators. |
| *observed entropy (plug_in)* | The observed entropy, with the **Raw (plug-in)** estimator. |
| *observed entropy (plug_in; fallback: support at or below threshold)* | The paper's estimator at an LS or ELS position. |
| *observed entropy (plug_in; fallback: too few points)* | An eligible position with support below 1.25 T (HIV-1 *pol* protein positions 42 and 43, support 105). |
| *observed entropy (plug_in; fallback: degenerate regression)* | T = 2 and a support of 3, where every subsample has the same size. |
| *observed entropy (plug_in; fallback: single observation)* | Chao–Wang–Jost or Miller–Madow at a position with one sequence (0 bits). |
| *no entropy (not_estimable)* | No support (NS). |

The fallback's names are those of `entropy_fallback` in the results, with spaces; see
[Entropy method](/reference/glossary/#entropy-method) and
[Entropy fallback](/reference/glossary/#entropy-fallback). Hover over **Method**, **Distinct
variants** or **Total variants** for a short explanation.

#### Similarity-weighted values

When the run computed similarity-weighted entropy, the Results screen also shows:

- a chip right after the *mean* chip, **similarity-weighted mean X bits (observed Y)**: the mean
  similarity-weighted entropy of the eligible positions and, in parentheses, the mean observed
  (uncorrected) entropy of the same positions, so the two are compared like for like. The *mean*
  chip before it is the run's entropy, estimated with the run's estimator (the paper's correction
  by default). For the MERS spike sample (`samples/mers_spike_aa.fasta`, default options with
  **Compute similarity-weighted entropy (Wu 2025)** ticked) the chip reads
  *similarity-weighted mean 0.0363 bits (observed 0.1207)*, next to *mean 0.1261 bits*: the
  similarity-weighted mean is about 30% of the observed mean of the same positions. With no
  eligible position it reads *similarity-weighted mean —*;
- a **Similarity-weighted** column in the position table, right after **Entropy** and sortable,
  like every column but **Coverage** (its tooltip says *Sort by similarity-weighted entropy*). In
  the exported positions table the column stays at the end, as `dima analyze` writes it;
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

#### Motif composition and variants

**Motif composition** is a bar of the position's supporting sequences by
[motif](/reference/glossary/#index-major-minor-and-unique), with its legend: *Index 75.9%*, *Major
8.2%*, *Minor 13.5%*, *Unique 2.4%* at MERS position 25 (see
[Motif composition](/reference/glossary/#motif-composition)). Hovering a part of the bar shows,
for example, *Index: 75.9% of the supporting sequences*.

**Variants** lists the position's variants (its distinct k-mers) with their **Count**,
**Incidence**, **Motif** and **Sequence** (the k-mer), most common first; *10 variants* at
position 25. Click a header to sort by it (*Sort by count*, …). To narrow the list:

- type into *Search k-mer or motif*: a part of a k-mer (any letter case: `kpac` finds
  `SVKPACIEV`), a part of a motif's name (`maj`), or a motif's code (`I`, `Ma`, `Mi` or `U`; a
  code also finds the k-mers that contain those letters);
- set **Minimum incidence** (0% shows all) to hide variants below that incidence.

The count then reads, for example, *1 of 10 shown* (after `kpac`), and **Clear** removes both
filters. Click a variant's row to
select it; from the keyboard, press Tab until its sequence is outlined and press Space or Enter.
Tab reaches the variants the table shows whole; to reach another, scroll the table, search for it
or sort the table. Right-click a row for **Copy k-mer** (the k-mer alone) or **Copy row**. The
first Index variant is selected when you select a position. A position without support says *No
variants: no sequence has a valid k-mer here*.

#### Metadata

When the run stratified variants by header fields, **Metadata of SFKSACIEV** shows the selected
variant's counts per value of each stratified field, in the order of the header format. Each field
is a row you can open or close (the first is open), with its number of values (*3 values*); an
open field lists its five most common values, each with its share of the variant's own sequences
and its count. At MERS position 25, stratified by host, the Major variant `SFKSACIEV` (14
sequences) reads *Camelus dromedarius 64.3% (9)*, *Homo sapiens 21.4% (3)* and *Camelus 14.3%
(2)*: these are the shares of the 14 sequences that carry it (its
[relative incidence](/reference/glossary/#relative-incidence) per value), not of all sequences.
Hover over a value for its count, *Camelus dromedarius: 9 of 14*. **Show all N values** lists up
to 200 (*N more not listed* counts the rest) and **Show top 5** goes back. Without a selected
variant the section says *Select a variant to see its metadata*; a variant without metadata says
*This variant carries no metadata*. See [Stratify by header metadata](/how-to/metadata/) for how
to read these counts.

### The filter bar

The filter bar decides which positions the chart and the table show:

- **Positions**: *from* and *to*, the first and last position shown.
- **Entropy** (or **Similarity-weighted entropy** while the chart shows it): *min* and *max*, the
  range of the metric's value shown, in bits. Positions without usable support have no value and
  are not affected.
- **Motifs**: **Index**, **Major**, **Minor**, **Unique**. A position is shown when at least one
  of its variants has a ticked motif; with all four or none ticked there is no motif filter. Any
  motif filter hides NS positions, which have no variants. On the MERS sample, unticking only
  **Index** shows *890 of 1345 positions* (those with a variant other than the Index), and
  **Unique** alone *737 of 1345 positions*.
- **Low support (NS, LS, ELS)**: one check box that shows or hides the three classes together
  (the [low-support](/reference/glossary/#low-support) positions). Unticked, the HIV-1 *pol*
  protein data set shows its 476 eligible positions.
- The count, *1345 of 1345 positions* (highlighted while filters hide positions), and **Reset**,
  which shows every position again.

A change applies when you release the mouse button, not during a drag. Applying it shows every
position on the chart again (the zoom is reset) and, if the selected position is now hidden,
selects the first position shown. The value axis follows the largest value shown, so a filter can
rescale it.

To change a bound of the value range, drag its field right or up to raise it, left or down to
lower it, or click the field, type a value and press Enter. Each pixel of a drag moves the bound
by 1/200 of the metric's full range, so 200 pixels cross the whole range; a range below 0.02 bits
moves by 0.0001 per pixel instead, and takes fewer. A typed value, in a value field or a position
field, applies when you press Enter or Tab or click elsewhere, not while you type, so the table
and the chart do not change at every digit; press Esc instead to leave the field as it was. The
value fields show four decimals, and a value you type is kept to four decimals, as the field shows
it: typed as the minimum, 0.111293 becomes 0.1113, so a position whose entropy reads 0.1113 in the
table but is 0.111294 is then hidden. A minimum that reads 0.0000 is 0, and a maximum that reads
as the largest value is that value exactly, so the full range hides no position. Each field shows
only its number, so it is named by what it bounds: *First position shown* and *Last position
shown* for the position fields, and the metric it filters for the value fields, for example
*Highest similarity-weighted entropy shown, in bits*. The name is the field's tooltip and what
screen readers announce. Nothing in the bar moves while you type in a field or when a number in
it gets shorter.

### The command palette

Ctrl+K (⌘K on macOS) opens the command palette, a field *Position number, k-mer, or command* over
the window:

- **A number** offers *Go to position 25*, when the results have that position. A position the
  filters hide has the note *hidden by the filters*; choosing it asks, in a dialog titled
  *Position hidden by the filters*: *Position 25 is hidden by the current filters. Show every
  position and select it?*, with **Reset filters and select** and **Keep as is**.
- **Two or more letters** find the positions where a variant contains them (any letter case),
  the first such variant of each position, at most 12 positions: *SFKSACIEV at position 25*, with
  its motif and incidence (*Major, 8.2%*).
- **A command**, found by any part of its name: **Open a file…**, **Switch light / dark theme**,
  **Cite DiMA**; with results loaded, **Go to the results** (on the Setup screen) or, on the
  Results screen, **Go to setup**, **Reset filters**, **Show every position (reset zoom)**,
  **Show similarity-weighted entropy** or **Show entropy** (when the run computed both),
  **Show diagnostics** and **Show run details**; and an **Export …** command for every export.

Up and Down move through the list, Enter runs the entry shown, and Esc or a click outside closes
the palette (*↑↓ move, Enter run, Esc close*). *No matches* means nothing fits what you typed.

### The diagnostics window

The badge in the top bar (or **Show diagnostics** in the palette) opens **Diagnostics**: every
condition the run and the conserved regions reported, each a [diagnostic](/reference/glossary/#diagnostic)
with its severity (*Warning*, or *Note* for information), its code, how many times it occurred
(*×42*), up to five examples, and a hint. For the MERS sample with a minimum coverage of 0.98:

```text
Warning  below_min_coverage  ×42
42 positions have sample coverage below 0.98
  • position 21 (coverage 0.9767)
  • position 22 (coverage 0.9767)
  • position 23 (coverage 0.9767)
  • position 24 (coverage 0.9767)
  • position 25 (coverage 0.9766)
Hint: their k-mer diversity is incompletely sampled; see sample_coverage
```

A run without any says *No conditions were reported for this run.* The codes, with their cause and
fix, are listed in [Diagnostics](/reference/diagnostics/). Hints name the command-line options;
the table [In the desktop app](/reference/diagnostics/#in-the-desktop-app) gives the control for
each (for example `--support-threshold` is **Support threshold T**, and `--validation permissive`
is **Advanced ▸ Validation ▸ Permissive**).

**Input validation**, closed at first, records how the input was read: **Alphabet** (*Protein
(detected; 236346 residues in 191 records examined)*, or *chosen* when you chose it),
**Contradiction** (when the residues look like another alphabet than the one chosen), **T/U
folded**, **Validation mode**, **Records**, **Lower case folded**, **Whitespace lines** (*0
trailing, 0 interior*), **Unknown characters**, **Header UTF-8 replacements** and, when variants
were stratified, **Header values cleaned** (trimmed, NFC-normalised, filled, invisible characters
removed), **Header fields** and **Mismatched headers** (with the first of them, record by record).
The table **Characters in sequence lines** lists every character found, in any mode: the
**Character** (*·* for one that cannot be shown), its **Byte**, its **Class** (residue, gap,
missing, ambiguity, stop, frameshift, whitespace or unknown; see
[Symbol class](/reference/glossary/#symbol-class)) and its **Count**: on the MERS sample `-` (gap)
22077 times and `B` (an ambiguity code) once, among the residues.

### Run details

**Run details** shows how an analysis made in the app ran, as its
[run report](/reference/glossary/#run-report) records it:

- **Equivalent command**, the `dima analyze` command that gives the same results, with **Copy
  command**: `dima analyze --input mers_spike_aa.fasta --threads 3` for default options on a
  4-CPU computer;
- **Started**, **Finished** and **Wall time**;
- **Worker threads**, *3 (settings)*, or *4 of 8 (settings; the system refused the rest)*;
- **Platform**, *linux x86_64, 4 logical CPUs*;
- the time of each phase (*opening*, *counting k-mers*, *computing statistics*) and **Waiting for
  input**: the time the counting threads waited for the file to be read;
- **Records**, **Positions**, **Similarity-weighted entropy** (*computed* or *not computed*),
  **Input** (*mers_spike_aa.fasta, 277 kB*) and its **SHA-256**;
- **Exported files**: the data files exported from these results so far, with their role and
  size (hover for the SHA-256), or *none yet*;
- **Save run report…**, and a note that CPU time and peak memory are not recorded, because the
  app's process also draws the interface (the command line records them).

For results opened from a file, the window says *These results were opened from NAME. Run details
are recorded for analyses run in this app; the results' own provenance:* and gives **Written
by**, **Input**, **SHA-256**, **Estimator** (as the command line spells it, `chao-wang-jost`) and
**Similarity-weighted entropy**.

## Export

**Export**, in the top bar while results are loaded, writes them (*Export (busy)* while an export
is being saved; one export runs at a time). Every item has a tooltip saying what it writes.

| Export | Writes | Same as |
|---|---|---|
| **Results (JSON)…** | The complete results with provenance; the app opens them again. | `dima analyze -o x.json` |
| **Positions (JSON Lines)…** | One position per line ([JSON Lines](/reference/glossary/#json-lines)); provenance and summaries are in the JSON. | `dima analyze -o x.jsonl` |
| **Tables (TSV folder)…** | The four [tidy tables](/reference/glossary/#tidy-table) `<name>.positions.tsv`, `<name>.variants.tsv`, `<name>.metadata.tsv` and `<name>.hcs.tsv` into the folder you choose, the conserved regions at the threshold shown. | `dima analyze --tsv-dir`, with `--hcs-min-incidence` at that threshold |
| **Conserved regions (JSON)…**, **(TSV)…** | The conserved regions at the threshold and switch shown. | `dima hcs` |
| **Entropy chart (PNG)…**, **(SVG)…** | The chart as shown (see below). | — |
| **Run report…** | How the analysis ran: version, equivalent command, threads, phase times, platform, and the size and SHA-256 of the input and of the data files exported so far. No paths, user or host names. Only for analyses run in the app (otherwise greyed out: *Available for analyses run in this app*). | `dima analyze --run-report` |
| **Methods & references…**, **References (BibTeX)…** | A methods paragraph for this analysis and its numbered references, or its references in BibTeX, the conserved regions at the threshold shown included. | `dima cite --from` with the results and the conserved regions |

The tables are named after the query name: `<name>` is the query name with every character but
letters, digits, `.`, `-` and `_` turned into `_` and leading or trailing dots and underscores
dropped (`dima` if nothing is left), as `dima analyze --tsv-dir` names them when it reads a file.
For results analysed from standard input, whose query name is *Unknown Protein*, the app names
them `Unknown_Protein.*.tsv` where `dima analyze --tsv-dir` writes `dima.*.tsv`. The other default
names below are made the same way.

### File names

The save dialog proposes a name made of the query name and the export: `mers_spike_aa.json`,
`mers_spike_aa.jsonl`, `mers_spike_aa.hcs.json`, `mers_spike_aa.hcs.tsv`,
`mers_spike_aa.entropy.png` (`.svg`), `mers_spike_aa.run-report.json`,
`mers_spike_aa.methods.txt` and `mers_spike_aa.references.bib`. The tables go into a folder you
choose (*Choose a folder for the tables*).

The data exports — results, positions, conserved regions and the run report — accept the names
the command line accepts for the same output, and are compressed as the name says:

- results, conserved regions (JSON) and the run report: `.json`, `.json.gz` or `.json.zst`;
- positions: `.jsonl` or `.ndjson`, either followed by `.gz` or `.zst`;
- conserved regions (TSV): `.tsv` or `.tab`, either followed by `.gz` or `.zst`.

The extension's letter case does not matter (`run.JSON.ZST`). A name without one of these endings
gets the export's usual extension added: `results` becomes `results.json`, and `results.gz`
becomes `results.gz.json` (plain JSON); to compress, end the name in `.json.gz` or `.json.zst`.
The figures and texts keep `.png`, `.svg`, `.txt` and `.bib`, added when missing. When the name
with its added extension is a file that exists, the app asks, in a dialog titled *Replace the
file?*: *NAME already exists in FOLDER. Replace it?*, with **Replace** and **Keep as is** (the
save dialog itself asked only about the name as you typed it).

Names the command line refuses are refused, and nothing is saved:

- a compression DiMA does not write: *Nothing was saved: '/data/x.json.bz2' asks for bzip2
  compression, which DiMA does not write (DiMA compresses with gzip or Zstandard: name it
  /data/x.json.gz or /data/x.json.zst)* (likewise `.xz`, `.zip`, `.7z` and the others; `.zstd`
  and `.gzip` are refused with the hint to use `.zst` or `.gz`);
- two compressions: *Nothing was saved: '/data/x.json.gz.zst' asks for two compressions (.gz,
  then .zst); DiMA applies one (name it /data/x.json.gz or /data/x.json.zst)*.

An export never replaces a file the app has loaded — the alignment the results were computed
from, the alignment on the Setup screen, or the results file they were opened from — however its
path is spelled, through a symbolic link included. Exporting **Results (JSON)…** of results opened
from `results.json` under that same name says *Nothing was saved: 'results.json' is the results
file these results were opened from; choose another name*. When the names differ only in letter
case, which macOS and Windows treat as one file, the message says the file *may be* that file. A
folder export is refused alike when one of its four tables would replace such a file.

Every file is written completely or not at all: an export goes to a temporary file in the
destination folder first and replaces the destination only when it is complete, so a failed or
interrupted export never leaves a partial file and never changes an existing one. A folder export
writes all four tables before any of them replaces a file, and when it fails, it removes the
folders it created. A notification says *Saved /data/mers_spike_aa.json* (or *Saved 4 files to
/data/tables*), or *Results (JSON) failed: …* with **Copy details**. If other results were loaded
while the dialog was open, nothing is saved: *Nothing was saved: other results were loaded while
the dialog was open. Export again to save the results now shown*.

### Figures

The entropy chart is exported as it is shown — the positions the filters show and the zoom — at
the size set in **Preferences**, 1800 × 640 pixels by default. The PNG is downsampled to the image
width with M4 (every peak and trough kept); the SVG has every point and editable text. The text
is the same size in both, and the figure carries:

- the title *mers_spike_aa — Shannon entropy per k-mer position*;
- the subtitle *191 sequences, k = 9, T = 100, estimator dima; 1345 of 1345 positions shown; DiMA
  1.2.0*, with the estimator as the command line spells it (`chao-wang-jost`);
- the value axis *Shannon entropy (bits)* (on screen *Entropy (bits)*) and the position axis
  *Position (alignment column of the k-mer start)*;
- the legend and the eligible-mean line of the chart, with its value.

The legend continues on further rows when it is wider than the figure, and a title, subtitle or
legend label too long for the figure is shortened with "…" (the SVG keeps the whole title as its
document title).

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
Likewise, if the filters hide every position while the dialog is open, nothing is saved and a
warning says *Nothing was saved: no positions are shown, so there is no chart to export (check
the filters)*; when they hide every position before you choose the export, the warning is *No
positions are shown, so there is no chart to export (check the filters)*. The other exports do
not depend on the chart and are saved as usual.

### Run report, methods and citation

The run report records the data files exported from the results so far (results, positions,
tables, conserved regions), with their size and SHA-256; figures, the methods text and BibTeX are
not recorded. For results opened from a file there is no run report (the palette's **Export Run
report** says *A run report describes an analysis run in this app; these results were opened from
a file*). The run report of a failed run is saved from the error card.

**Help ▸ Cite DiMA…** gives the citation of the paper (in text, *Tharanga et al. 2025*, and in
full, with its link) and of this version of the software. **Copy citation** copies both;
**Copy BibTeX** copies the BibTeX of the paper and of the software only. **Export ▸ References
(BibTeX)…** writes every reference the analysis used, as `dima cite --from` lists them, and
**Export ▸ Methods & references…** the methods paragraph with them.

## Notifications and questions

Notes, warnings and errors appear in the bottom-right corner, the newest nearest the corner, at
most four at a time (*+N more* above them counts the others). Each starts with its kind, *Note:*,
*Done:*, *Warning:* or *Error:*, so the kind never depends on colour alone; a message raised again
is counted (*(×3)*) instead of repeated. A long message wraps onto more lines and pushes the older
ones up, so every notification stays in the window. A note, or a message that something finished,
closes by itself after 4 to 12 seconds (longer for a longer message); a warning or an error stays
until you press its **×** (*Dismiss*), and an error also offers **Copy details**. The app keeps at
most 30 of them.

Questions come in a dialog with the action and **Keep as is**:

| Question | Asked when |
|---|---|
| *An analysis is running* (**Cancel analysis and open**) | You open a file while an analysis runs. |
| *Position hidden by the filters* (**Reset filters and select**) | You go to a position the filters hide. |
| *Replace the file?* (**Replace**) | The name with its added extension is a file that exists. |
| *An export is being saved* (**Quit without saving it**) | You close the window, or press Ctrl+Q, while an export is being saved: *Quit now? The file being saved will not be written. Keep DiMA open to let the export finish.* |

[Troubleshooting](/how-to/troubleshooting/) gives the cause and fix of every message.

## Keyboard shortcuts

**Help ▸ Keyboard shortcuts** lists them:

| Keys | What they do |
|---|---|
| Ctrl+K (⌘K on macOS) | Command palette: go to a position, find a k-mer, run a command |
| ← ↑ / → ↓ | Previous / next row of the position table |
| Home / End | First / last row |
| Tab / Shift+Tab | Move between controls |
| Space / Enter | Press or select |
| Enter | In a filter field: apply the value typed (Tab or a click elsewhere does too) |
| Right-click a table row | Copy row, or a variant's k-mer |
| Scroll over the chart | Zoom about the cursor |
| Drag / Shift-drag | Pan / zoom to a region |
| Double-click the chart | Show every position |
| Esc | Close the palette; in a filter field, leave it unchanged; also gives the arrow keys back to the table |
| Ctrl+Plus / Ctrl+Minus / Ctrl+0 (⌘+ / ⌘− / ⌘0) | Make the whole interface larger / smaller / its usual size (Ctrl+=, ⌘=, also makes it larger) |
| Ctrl+Q (⌘Q) | Quit DiMA |

The arrow keys, Home and End move along the table on the Results screen, while no text field has
the focus and the palette is closed. The interface zoom changes the size of everything in the
window, in steps of a tenth, and is remembered with the window (see
[Where the app keeps its settings](#where-the-app-keeps-its-settings)). Ctrl+Q closes the window
as its close button does, so it asks first while an export is being saved. On macOS, ⌘Q is the
**Quit** item of the app's menu, which macOS handles itself: DiMA quits without asking, and an
export still being saved is not written (the file it would have replaced keeps its contents).

## Theme, Preferences and About

**Theme.** At first the app follows your system's light or dark setting (it starts dark when the
system does not tell it). The sun or moon button at the right of the top bar switches between the
light and the dark theme; its tooltip, and what screen readers say, is *Switch to light mode* or
*Switch to dark mode*. The palette's **Switch light / dark theme** does the same. Once you switch,
the app keeps your choice, also at the next start, and no longer follows the system.

**Preferences**, in the top bar, holds settings of the app rather than of an analysis, saved as
soon as you change them:

- **File dialogs**: **The system's file dialog** (it falls back to DiMA's own browser when the
  system dialog is not available) or **DiMA's built-in browser** (for remote desktops and systems
  whose file dialog does not respond).
- **Exported figures**: the width and height of exported figures, in pixels (drag a value, or
  click it and type), 1800 × 640 px by default. They are at least 720 × 320 px, so the title,
  legend and axes always fit, and at most 20,000 × 20,000 px; a value typed outside these limits
  is set to the nearest one.

A smaller figure size saved by an earlier version is raised to the minimums when this version
first starts, with a note; for a saved 1800 × 150 px: *Exported figures are now at least
720 × 320 px so the title, legend and axes fit; yours was raised to 1800 × 320 px*. A size above
the maximum, possible only in a settings file edited by hand, is lowered to it; for a saved
30,000 × 640 px: *Exported figures are between 720 × 320 and 20000 × 20000 px; yours was changed to
20000 × 640 px*. The corrected size is saved at once, so either note appears only once.

**Help ▸ About DiMA…** gives the version (*DiMA desktop app 1.2.0*), the platform it was built for
(for example *Built for x86_64-unknown-linux-gnu*: 64-bit Linux on Intel or AMD processors), the
links **Documentation** (these pages, for this version of DiMA) and **Source code**, and the method
it implements (*Implements the method of Tharanga et al. 2025, doi:10.1093/bib/bbae607.*). Below
them, three sections open on a click: **Licence (MIT)**, DiMA's licence; **Third-party notices**,
the code, fonts, data and Rust libraries that DiMA includes, with their licences, in parts that
also open on a click; and **IBM Plex fonts (SIL Open Font License 1.1)**, the licence of the
app's typeface.

## Where the app keeps its settings

The app keeps three files, in your user folders:

| File | Linux | macOS | Windows |
|---|---|---|---|
| `settings.json` and `recent.json` | `~/.config/dima-gui/` (or `$XDG_CONFIG_HOME/dima-gui/`) | `~/Library/Application Support/dima-gui/` | `%APPDATA%\dima-gui\` |
| `app.ron` | `~/.local/share/dima-gui/` (or `$XDG_DATA_HOME/dima-gui/`) | `~/Library/Application Support/dima-gui/` | `%APPDATA%\dima-gui\data\` |

- `settings.json` holds the remembered analysis options (see
  [Changed options and Reset to defaults](#changed-options-and-reset-to-defaults)), the worker
  threads and **Preferences**. It is written in the background whenever a setting changes, each
  time completely or not at all. A write that fails (for example in a folder you may not write
  to) is not reported: the app goes on, but the change is not remembered at the next start.
- `recent.json` holds the ten most recent files: their full paths, when you opened them, and what
  they hold (results, or the number of sequences and the alphabet). Press **Clear all**, or delete
  the file, to forget them. A file that cannot be read is taken as an empty list.
- `app.ron` is written by the windowing library (eframe): the window's size and place, the theme,
  the interface zoom, which sections are open (such as **Advanced**) and how far the panels are
  scrolled. **Reset to defaults** does not change it; delete it to start with the defaults.

If `settings.json` cannot be used, the app starts with the default options and says why, at every
start until you change any setting (which saves new settings in place of the old ones): *Settings
were reset to the defaults (the settings file could not be read).*, *(they were saved by a
different version of DiMA)* or *(the saved values were not valid)*. The file is not changed
meanwhile, so the version that wrote it can still read it.

Removing the app does not remove these files; delete the folders to remove them too.

## Screen readers

The app draws its interface itself and describes it to screen readers through AccessKit (on
Windows, macOS and Linux). Every control has a name, and what its tooltip shows is its
description, read after its name:

- each analysis option is named by its label and described by its tooltip, references included;
  the dot after a changed option reads, for example, *Support threshold T changed; default: 100*;
- the controls that show only a value are named by what they set: *Path of a file to open*,
  *Field 1 name*, *Field 1 custom name*, *Stratify field 1*, *Header format to paste*, *Minimum
  incidence*, *Command palette*, *First position shown* and *Last position shown*, *Lowest entropy
  shown, in bits* and *Highest entropy shown, in bits* (with *similarity-weighted entropy* in
  place of *entropy* while the chart plots it), *Width of exported figures* and *Height of
  exported figures*;
- the conserved-region threshold's two controls have names of their own: *HCS threshold (Index
  incidence), slider in 0.5% steps* and *HCS threshold (Index incidence), exact value in percent*;
  the worker-threads slider is *Worker threads*;
- the theme button is *Switch to light mode* or *Switch to dark mode*, a recent file's **×** is
  *Remove mers_spike_aa.fasta from recent files*, and a notification's **×** is *Dismiss*;
- the chips of the Results screen, the facts **Method**, **Distinct variants** and **Total
  variants** and the similarity-weighted facts, the legend entries, and the column headers (*Sort
  by entropy*) read their explanations;
- the chart describes itself, in the words of its legend: on the MERS data set *Entropy chart:
  1345 positions shown, 0 with support ≤ 100, 0 spans with no usable support*; on the HIV-1 *pol*
  protein data set *Entropy chart: 1189 positions shown, 162 with support ≤ 100, 39 spans with no
  usable support* (the 162 are the positions drawn with support ≤ 100; the 551 positions with no
  usable support are in the 39 spans). The strip does too: *Conserved regions: 44 regions, 0
  flagged*;
- the metadata fields read *Host, 3 values, expanded* (or *collapsed*), and their values
  *Camelus dromedarius, 9 of 14*: the value, then its count of the variant's sequences (here
  `SFKSACIEV` at MERS position 25).

A count of one is read in the singular: *1 region*, *1 value*, *1 span*.

The chart's points and the strip's regions are reached by the mouse only: their tooltips are not
read. Use the position table instead, with the arrow keys, Home and End (press Esc first when a
text field has the focus), and the command palette to go to a position (*25* then Enter) or to
find a k-mer; the selected position's facts are then in the panel on the right. The values of the regions' tooltips are in the exported conserved
regions (**Export ▸ Conserved regions (TSV)…** or **(JSON)…**).

## References

[^ref]
