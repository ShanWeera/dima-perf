# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). The
"Methodology" group lists every change that can alter a result, with the sources behind it.

## [1.2.0] - 2026-11-27

No computed value changes: the same inputs and options give the same entropies, incidences,
coverages, motifs and conserved regions as 1.1.1. Results files differ from 1.1.1's in the
version they record, in their references (`run.references` adds Chao, Wang & Jost 2013 for the
coverage, see "Changed") and, where a run records one, in its diagnostics: the new warning
`uncorrected_eligible_positions` and the reworded `threshold_exceeds_sequence_count`,
`alphabet_contradiction` and header hints. The results schema stays 1.1.0 (a diagnostic's `code`
is a free string), so 1.1.x readers read these files. This minor release adds warnings and checks;
some output names that 1.1.1 accepted are now refused because they misstated what was written
(see "Changed").

### Added

- Warning `uncorrected_eligible_positions` (with the paper's estimator, the default): eligible
  positions that kept their observed entropy because the paper's correction could not be
  computed there. The correction needs five regression points (a minimum taken from DiMA v5.0.9),
  so support of at least ⌈1.25 T⌉ (`too_few_points`); and at T = 2 with support 3 every subsample
  has one size (`degenerate_regression`). Positions with a single k-mer, 0 bits either way, are
  mentioned in the message but not counted, and alone give no warning. The hint gives the support
  needed and the largest threshold that would correct them (⌊0.8 × support⌋). It is printed on
  standard error, recorded in `run.diagnostics` and shown in the desktop app's diagnostics;
  results of earlier versions are not re-checked for it. Example and fix: Diagnostics and
  Troubleshooting.
- Warning `output_format_assumed` (command line): conserved sequences written to a name that
  names neither JSON nor TSV (`dima hcs -o regions.txt`, `--hcs-output regions.txt`) are JSON, as
  documented, and the warning says so, with the names to use.
- Library: `output::read_name` (what an output's name says: format, compression, or why it is
  refused), `output::written_in_place` (the names written as streams), `output::folded`,
  `output::one_file` and `output::why_one_file` (whether names that differ only in letter case or
  Unicode form are one file), `output::unreadable_results_compression`, and
  `entropy::min_corrected_support` and `entropy::max_correcting_threshold` (the exact rule of the
  correction, ⌈1.25 T⌉ and ⌊0.8 n⌋, 1 for n = 3): one rule each for the command line, the desktop
  app and the messages.
- Desktop app: *Help ▸ Keyboard shortcuts* lists the interface zoom (Ctrl/⌘ +, −, 0) and
  Ctrl/⌘ Q. Screen readers hear every tooltip as its control's description, distinct names for
  the conserved-region slider and its exact field, the worker-threads slider, the figure width and
  height and the minimum-coverage check box, and the theme switch in words; counts read
  "1 region", "1 value", "1 column". The k-mer length field steps by one with Up and Down. On
  macOS the Open dialog offers every file, so an alignment saved as `.txt` can be opened.
- Documentation:
  - Glossary: one entry per concept, with every name it has in the desktop app, the command
    line and the files.
  - Desktop-app guide, rewritten in the order a user meets the app: a first analysis step by
    step, then every card, control, chip, fact, window and export, the keyboard shortcuts, where
    the app keeps its settings and what screen readers hear.
  - Analyse an alignment: "Choose the support threshold" (a corrected entropy needs support of
    at least 1.25 T, so T at most 0.8 N) and "Compare two alignments": which numbers to compare,
    conservation classes, the effective number of variants, and checks for alignments of
    different size (Zhao et al. 2019, Chao & Jost 2012, Chao et al. 2014, Chao et al. 2013).
  - Entropy and its correction: where the correction applies, the fallback names, and what an
    entropy value means in size, as an effective number of variants, 2^H (Chao et al. 2014).
  - Conserved sequences: what a region's numbers mean (member positions, whole-region carriers,
    overlapping regions), reading regions below 100% (Hu et al. 2013, Raman et al. 2020, Bui et
    al. 2007), and tested recipes for reference coordinates and whole-region carriers.
  - Metadata: reading counts with care (Tharanga et al. 2025, Brito et al. 2022, Hu et al. 2013,
    Raman et al. 2020) and tested recipes for the background mix at a position and for dates
    reduced to years.
  - Large alignments: "Read the -v report". Clusters: a Slurm array, a Snakemake rule, failures
    listed from run reports. Reproducibility: comparing with DiMA v5.0.9. Install: shell
    completions, manual pages and what a release contains. FAQ: choosing T for small data sets
    and the option map from dima-cli 5.0.9. Troubleshooting and Diagnostics: cause and fix for
    every message of the command line and the desktop app.
  - The version picker in the header of every page names the exact release the pages describe
    (`v1.2.0`), also offline, where the list of versions cannot load.

### Changed

- Output names, on the command line and in the desktop app: a name that names a compression
  DiMA does not write (`.bz2`, `.bgz`, `.xz`, `.lzma`, `.lz`, `.lz4`, `.z`, `.br`, `.sz`, and
  `.zstd` or `.gzip` for `.zst` or `.gz`) or an archive (`.zip`, `.7z`, `.tar`, `.tar.gz`,
  `.tar.zst`, `.tgz`, `.tbz2`, `.txz`), two compressions, a run report named as a table (`.tsv`,
  `.jsonl`), or a folder (a name that ends in a path separator, or whose last part is `.` or
  `..`) is refused with exit status 2 before anything runs, with the name to use. A name DiMA
  suggests keeps the name given: it adds the format's extension to a name that names none
  (`regions.txt` → `regions.txt.json`) and puts the output's own name in place of another
  format's extension (`run.tsv` → `run.run-report.json`, `x.jsonl` → `x.hcs.json`), so it is
  never the results' usual name.
- The desktop app's exports accept the command line's names (`.ndjson.gz`, `.tab`, a run report
  named `.json.gz`, written compressed) and refuse the ones it refuses.
- A folder given as an output, or a file given as `--tsv-dir`, is a usage error (exit status 2)
  on every platform.
- Every file and folder a command writes is checked against every other and against the input
  before anything runs, the run report first, however the names are spelled (`./`, `..`,
  symbolic links). Names that differ only in letter case, or in the Unicode form of an accented
  letter, are refused unless the file system shows two files (only one exists, or their inodes
  differ). On Unix, standard input and standard output are compared with the files a run replaces
  by device and inode, so `dima analyze -o x.fa < x.fa` and `-o r.json --hcs-output - > r.json`
  are refused, and so is `dima cite --from r.json >> r.json`. Two outputs that lead to one pipe,
  FIFO or socket (`-o /dev/fd/3 -O json --hcs-output - 3>&1 | …`, a link to `/dev/stdout` next to
  `-`) are refused. Devices such as `/dev/null` take any number of outputs, and appending standard
  output to a log (`>> log`) is never a clash. `dima view --tsv-dir` checks the names it will
  write. Messages name the outputs in words ("the positions table (--tsv-dir)").
- The run report is opened right after its check, so a report that cannot be written fails
  before the run. A failed run removes every folder level it created, except one that holds its
  run report.
- Outputs are written in a fixed order: files first, then devices, FIFOs and other descriptors,
  and standard output last.
- `DIMA_PROGRESS` is read by `dima analyze` itself: an empty value counts as unset, and a value
  that is not `auto`, `plain` or `off` is a usage error. An explicit `--progress` wins over
  `DIMA_NO_PROGRESS` (before, the variable won), and of `--progress` and `--no-progress` the last
  one given wins.
- `dima cite --from` refuses results and conserved sequences of different runs, saying which
  value is whose ("T is 100 in 'a.json' and 4 in 'b.hcs.json'"), and `--from -` given twice.
  `--licenses` with a subcommand is a usage error.
- The `Cite:` line names the results file only when it is a regular file, quoted for the shell
  where needed; otherwise it suggests `dima cite`.
- An interruption's notice names the signal and how to stop at once: "press Ctrl-C again" when
  a terminal is attached, otherwise "cancelling on SIGINT... (a second signal stops
  immediately)", as for every signal another program sends.
- Messages: `threshold_exceeds_sequence_count` is worded for the estimator used and its hint
  gives the largest threshold that corrects (⌊0.8 N⌋, 1 for N = 3), without the repeated example it and
  `alphabet_contradiction` gave; when no header matches `--header-format`, the hint says the
  format is probably wrong before suggesting `--validation permissive`; a residue that is not
  valid in the alphabet given with `-a` suggests checking `--alphabet` first when the first
  records look like another alphabet; the k-mer length error names the default k when `-k` was
  not given ("27 (the default for DNA and RNA)") and says "1 column"; `--run-report -` alone
  explains that the results already go to standard output; the hint for unreadable compressed
  results mentions `dima analyze` only for bzip2, the one of those formats it reads alignments
  in; a run report clashing at a device or descriptor suggests `run-report.json`; the folder hint
  gives a file name for that output (`out/hcs.json`) and mentions `--tsv-dir` only where the
  command has it; `dima hcs` names its default output "(standard output by default)", as
  `dima analyze` does.
- The methods paragraph of `dima cite --from` and the desktop app's *Methods & references*
  define supported positions, state that the paper's correction applies where support is at
  least 1.25 T (credited to DiMA v5.0.9), give a conserved region's bound as the lowest
  per-position 95% Clopper–Pearson lower bound among its members, credit the coverage's form
  where no k-mer is seen exactly twice to Chao, Wang & Jost 2013 (Materials and methods), which
  the run's references now list, and add sentences for the detected alphabet (Easel) and the
  IUPAC-IUB codes when the run used them, so every reference it lists is cited.
- Citations: Eddy 2011 is cited for HMMER only; the gap, missing-data and white-space conventions
  and the alphabet detection DiMA takes from Easel are cited to Easel's source code at the ported
  commit. New references for the reading guidance: Chao et al. 2014, Brito et al. 2022 and Bui
  et al. 2007. `CITATION.cff` lists Castresana 2000 (conserved regions of eligible positions
  only).
- Help texts: `-s` and `--estimator` say that the paper's correction applies where possible and
  define N, n and D; `-q` (`-qq` is `-q`; it also hides the `--validation report` table and the
  `Cite:` line), `-v`, `--progress`, the `--tsv-dir` names of each command, the conserved-region
  options, `dima hcs -o`/`-O`, `--hcs-output`, `--run-report` and `cite --from` say exactly what
  they do; values of every option, `--color` included, are lower case. The CLI reference adds
  `-h`, `-V`, `dima help`, the meaning of each value, accepted alternative spellings,
  environment variables and the 128 + N exit statuses.
- Desktop app: the threshold and estimator tooltips, the chart's encoding and the T chip say that
  the paper's correction applies where possible; the *Method* fact and the chart's tooltip name
  the method in words and as the file writes it ("paper's correction (dima)"); figure subtitles
  and *Run details* spell the estimator as the command line does (`chao-wang-jost`); the
  *Alphabet* line gives detection's own reason, only while the alphabet is *Auto*; problems under
  **Analyze** and thread messages name the app's own controls; the validation tooltip says that
  *Report* differs from *Permissive* only on the command line. The conserved-region and
  worker-threads sliders no longer draw a value box of their own: the exact field shows the
  threshold, and "N of M CPUs" follows the threads slider. The minimum-coverage tooltip names
  both coverage sources. Save dialogs list the format's usual extension first, so a name typed
  without one gets `.json`, not `.gz`, on Windows and macOS. An export is refused over a loaded
  file whose name differs only in letter case only when the file system cannot tell the two
  apart.
- The Bioconda build sets `DIMA_RELEASE=1`.

### Fixed

- Data loss: a run report named like one of the `--tsv-dir` tables could replace an existing
  table of that name when the run failed before its tables were checked (an invalid option, an
  output name that names no format). Standard input or output redirected to a file a run
  replaces could lose that file too (`-o x.fa < x.fa`, `-o r.json --hcs-output - > r.json`), and
  `dima cite --from r.json >> r.json` appended the citation to the results, which were then no
  longer valid JSON. All are refused before anything runs, and a refused run report is not
  written.
- Two outputs written into one pipe or descriptor (`-o /dev/fd/3 -O json --hcs-output /dev/fd/3`)
  were mixed into one stream that neither format could read; they are refused.
- An interruption while the input was still empty (an idle pipe or FIFO) could not be noticed
  until the 3-second grace period ended the process, without the closing message or the run
  report; the input is now opened on a thread of its own, and the interruption is handled at
  once.
- A name longer than file systems allow (255) and a symbolic-link loop failed only after the
  analysis, when the finished output was renamed into place (a looped link was even replaced);
  both now fail before the analysis. A `--tsv-dir` that is a file, with the run report inside it,
  is a usage error instead of a failed folder creation (exit status 3).
- The time the counting threads waited for the input (`waited_for_input_seconds`, and the `-v`
  note "reading the input limited the run") included the first batch, which is read while
  opening; it now counts the counting phase only, and is null only when the analysis itself did
  not finish.
- Results compressed with bzip2, xz, LZ4, zip, 7z or another format DiMA does not read results
  from were reported as "not a DiMA results file"; `dima view`, `dima hcs`, `dima cite --from`
  and the desktop app now name the format and what to do. Results compressed by pzstd (which start with a skippable
  frame) are read.
- A closed output pipe (`dima analyze … | head`) now prints the run's warnings and summary line
  before exit status 0, and the run report lists the outputs that were committed.
- An interruption: the closing message is printed with `-q`, names the outputs that were complete
  and says when standard output may hold a partial document; the run report lists the committed
  outputs.
- The examples of `below_min_coverage` and of the conserved-region bounds never read as reaching
  the bound they are below. `uncorrected_eligible_positions` speaks of one position in the
  singular, and `threshold_exceeds_sequence_count` with one sequence no longer says that
  Chao–Wang–Jost or Miller–Madow still applies.
- Desktop app: a default k longer than a short alignment was silently replaced by the
  alignment's length; it is now shown as it is and named under **Analyze**, which stays
  disabled. The chart's text and the support-class line said that Chao–Wang–Jost and
  Miller–Madow estimate a position with one sequence (it keeps its observed entropy, 0 bits).
  Results compressed with bzip2 and other formats DiMA does not read results from were sent to
  the alignment pre-check; they are named for what they are, and bzip2 alignments still open. A
  coverage below the run's minimum never reads as reaching it. Exports never replace a file the
  app has loaded, and a failed folder export removes the folders it created. A one-column
  alignment reads "1 column", one eligible position "1 position is eligible", and T = 1 "more
  than 1 sequence (T)".
- `scripts/validate_paper.sh`, `scripts/bench/run.sh` and the two converters print their usage
  on `--help` and exit 0.
- Documentation: wrong statements corrected, among them what Eddy 2011 describes, which
  datasets the repository holds ("the paper's four datasets in the repository", not "bundled"),
  the internal-error claim of Troubleshooting, `threads_exceed_cpus`, "`dima view` converts the
  same way" for table names, the name of the Linux `.deb` package (`di-ma`, so `sudo apt remove
  di-ma`), the section of Khan et al. 2008 that holds the upper-bound quotation (Methods), what
  the quotation of Brito et al. 2022 refers to, which files `SHA256SUMS` lists, when
  `--run-report -` is possible, quoting in the Windows Command Prompt, and that `--color` values
  are case-sensitive like every other.

## [1.1.1] - 2026-11-26

No result changes: the same inputs and options give the same values as 1.1.0; only the version
recorded in the outputs differs.

### Changed

- Documentation: the *eligible mean* is now defined and explained. It is the mean entropy of the
  eligible positions (support above T), each position counted once: `average_entropy` in the
  results, `entropy mean` in the end-of-run line, the desktop app's *mean* chip and the dashed
  line labelled "eligible mean" on its chart, which shows the eligible mean of
  similarity-weighted entropy when the chart does. The Glossary and the FAQ define it;
  "Diversity motifs and summaries" gives its formula, a worked example on the HIV-1 *pol* protein
  data set that compares it with the other averages of the same run (1.9072 bits, against 1.5708
  over the positions the chart draws) and how it changes with the estimator and T; the
  desktop-app guide explains how to read the line ("Read the eligible mean line": zooming and
  the filters do not change its value, a filter can rescale the value axis and so move it,
  positions with support ≤ T are not in it, and when it is not drawn); "Your first analysis",
  "Analyse an alignment" and the JSON reference name it where they describe `average_entropy`
  and the end-of-run line; and "Choose and read outputs" shows how to draw the same line from
  the positions table in R and Python.
- Desktop app: the tooltip of the chart's *eligible mean* legend entry says which positions the
  line averages (support > T, each counted once), that it is the value of the *mean* chip (with
  similarity-weighted entropy, the first number of the similarity-weighted chip) and that
  zooming and the filters do not change it, with its reference; it said only "Mean entropy over
  the eligible positions (support > T), as in the summaries". The tooltips of the *mean* and
  similarity-weighted chips name the line, count "1 eligible position" in the singular, and,
  when no position is eligible, say that there is no mean and no line.
- Desktop app: screen readers read each legend entry of the chart with the explanation its
  tooltip shows, as they do the chips; they read only its caption before.

## [1.1.0] - 2026-11-26

### Methodology

- **Similarity-weighted entropy (opt-in)**: Wu J. 2025, "Similarity-weighted entropy for
  quantifying genetic diversity in viral quasispecies", *Virus Evolution* 11(1):veaf029
  ([doi:10.1093/ve/veaf029](https://doi.org/10.1093/ve/veaf029)), Section 2.1 and
  Supplementary File 1. Each variant's entropy term is weighted by its dissimilarity to the rest
  of the position, the similarity of two k-mers being their proportion of identical residues
  (Grishin 1995, [doi:10.1007/BF00175826](https://doi.org/10.1007/BF00175826), p. 675). Also the
  observed and similarity-weighted entropies normalised by log2 of the number of distinct
  k-mers (the Index included); undefined with one distinct k-mer. The weights' sums are obtained
  exactly from residue counts per site instead of pairs of variants (the column-profile form of sum-of-pairs scores,
  Gotoh 1994, [doi:10.1093/bioinformatics/10.4.379](https://doi.org/10.1093/bioinformatics/10.4.379),
  eqs. (1) and (4a)). Without the option nothing is computed and no existing value changes;
  with it, no existing value changes either.

### Added

- `dima analyze --similarity-weighted-entropy` computes similarity-weighted entropy and the two
  normalised entropies (see "Methodology"); the end-of-run line then adds
  `similarity-weighted mean X bits (observed Y)`, the mean observed entropy of the same eligible
  positions for a like-for-like comparison. Without an output that holds the values (results
  JSON or JSONL, or the positions table) the option warns (`flag_has_no_effect`) and is not
  computed.
- Results: `similarity_weighted_entropy`, `normalised_observed_entropy` and
  `normalised_similarity_weighted_entropy` per position, `similarity_weighted_entropy_summary`
  (eligible positions, with their mean observed entropy) and the parameter
  `run.parameters.similarity_weighted_entropy`; three columns appended to the positions table.
  `dima view`, `dima hcs`, `dima cite --from` and the desktop app refuse a file whose values break
  the rules of the results reference (present exactly where they are defined, and no similarity-weighted
  value above the observed entropy) as edited or damaged (exit 4).
  Without the option the keys are `null` (`NA` in the table); default results grow by 116 bytes
  per position (compact JSON and JSONL; 140 indented; 9 per positions-table row), and each
  position takes about 48 bytes more memory until the results are written.
- Desktop app: **Compute similarity-weighted entropy (Wu 2025)** in the Advanced options, off by
  default, remembered between sessions and turned off by **Reset to defaults**. The closed
  Advanced section's title counts the options inside it that differ from their defaults
  ("Advanced (1 changed)"), and **Run details** says whether similarity-weighted entropy was
  computed, for analyses run in the app and for results opened from a file. The new setting is
  saved only when it is on, so default settings stay readable by dima 1.0.0.
- Desktop app: when the run computed similarity-weighted entropy, a **Metric:** switch above the
  chart plots **Entropy** or **Similarity-weighted entropy**. Each has its own value axis,
  heading, legend and mean line, and the filter bar's value range applies to the metric shown. A
  switch keeps the zoom and the selection and resets the value range (with a note, and the
  position table scrolled back to the selected row, when it had been narrowed); no control
  moves. Every dataset opens showing entropy. Exported figures show the metric on screen, with
  its title, axis title, legend and mean.
- Desktop app, when the run computed similarity-weighted entropy: a chip "similarity-weighted
  mean X bits (observed Y)" after the mean chip (0.0363 and 0.1207 bits for the bundled MERS
  spike sample); a sortable **Similarity-weighted** column right after **Entropy** in the
  position table; and the selected position's similarity-weighted and normalised entropies in
  its facts and in the chart's tooltip, the normalised values marked "(low support)" at support
  ≤ T and reading "— (one variant)" where a single k-mer leaves them undefined.
- Desktop app: the position table scrolls sideways when its columns do not fit the window, and
  every sortable header has its "Sort by …" tooltip as its accessibility description.
- Desktop app: the chart exports follow the metric shown. A similarity-weighted figure is
  offered as `<query>.similarity-weighted-entropy.png` (or `.svg`), and its menu items read
  "Similarity-weighted entropy chart (PNG)…"/"(SVG)…". A figure is not saved if the chart was
  switched to the other metric while its save dialog was open; a warning says so. The command
  palette offers "Show similarity-weighted entropy" or "Show entropy", whichever the chart does
  not show, and the keyboard shortcuts list Tab / Shift+Tab, Space, and that Esc gives the arrow
  keys back to the table.
- Documentation: the methodology of similarity-weighted entropy on "Entropy and its correction"
  (the definition, the similarity of two k-mers, the weights and their bounds, the normalised
  values, a worked example at MERS-CoV spike position 25, how the values depend on k, the alphabet
  and the number of sequences, with two tables of exact averages, what the evidence covers, and
  where the paper and DiMA differ), a Wu 2025 section on "Comparison with the papers", and the
  option in the guides, the references, the FAQ, the Glossary and Troubleshooting.
- Test data: Wu 2025's ToBRFV samples (Supplementary File 2, CC BY 4.0) as
  `tests/data/tobrfv_tm2.fasta.bz2` and `tobrfv_tm22.fasta.bz2`, written by
  `scripts/reference/tobrfv_from_supplement.py`, and the ten PSTVd loop-27 libraries of Wu 2025's
  Fig. 5c (the S2 Table of Wu et al. 2024, PLOS Pathogens,
  [doi:10.1371/journal.ppat.1012142](https://doi.org/10.1371/journal.ppat.1012142), CC BY 4.0) as
  `tests/data/pstvd_loop27_<library>.fasta.bz2`, written by
  `scripts/reference/pstvd_from_supplement.py`; "Comparison with the papers" compares DiMA's
  values on them with Fig. 5c. The reference implementation
  (`scripts/reference/dima_reference.py`) computes similarity-weighted entropy pair by pair, as Wu
  defines it, with `--similarity-weighted-entropy`.
- `scripts/reference/docs_numbers.py` computes the documentation's numbers that come from
  expectations and simulations rather than from a dima run, exactly wherever an exact form exists
  and otherwise by seeded Monte Carlo, and writes them into the pages and
  `scripts/validate_paper.sh`; CI fails when they disagree.

### Changed

- Desktop app: the filter bar's value fields show four decimals (two to four before, depending
  on the value) and keep their width as their values change. Dragging a value moves it by 1/200
  of the range per pixel, so the whole range takes 200 pixels whatever the metric's scale, but
  by at least 0.0001 per pixel, so a range below 0.02 bits takes fewer; it moved 0.01 bits per
  pixel before, too coarse for similarity-weighted entropy, which is often a small fraction of a
  bit.
- Results schema 1.1.0 (the conserved-sequences schema stays 1.0.0 and now has its own version
  constant). Results written by dima 1.0.0 are read as runs without similarity-weighted entropy;
  `dima view` notes this when it writes values their schema did not record, and keeps its
  `newer_schema` warning for newer or unrecognised versions, now quoting a version that is not a
  plain `MAJOR.MINOR.PATCH` (so that, for example, a stray space shows).
- Documentation: the methodology page explains how DiMA applies Wu's similarity, defined and
  tested on nucleotide sequences, to amino-acid k-mers: the same identity-based computation,
  which residues are compared, why identity is used rather than a substitution matrix, and how to
  read and compare the values ("Amino-acid k-mers (DiMA's extension)").
- Documentation: "Comparison with the paper" is now "Comparison with the papers" (same address),
  since it also compares DiMA with Wu 2025.
- Desktop app: a value typed into a field of the filter bar (a position or a bound of the value
  range) is applied when you press Enter or Tab or click elsewhere, no longer at every key, and
  Esc leaves the field as it was. Typing 0.2 as the maximum applied 0 at the first key, which
  hid the selected position and moved the selection away for good. Dragging and the Up and Down
  keys still apply at once.
- Desktop app: the cells of the position and variant tables are no longer keyboard stops. Tab
  stopped at every cell without showing it (16 cells before the metric switch on the bundled
  MERS spike sample, 52 on the HIV-1 pol protein data set). Positions move with the arrow keys
  as before; a variant the table shows whole is one stop, on its sequence, which is outlined, and
  Space or Enter selects it, as does a click on the sequence (a click on the text did nothing). A
  column header, the chart and the strip of conserved regions are outlined too while they have
  the keyboard focus (the chart and the strip showed nothing), also at the clipped edge of a
  table.
- Desktop app: a click anywhere in a row of the position or variant table selects it, its text
  included; the text in the tables can no longer be selected, which took the click (a click on
  the "5" of position 5 left the selection where it was, in 1.0.0 too). Instead, right-click a
  row to copy it: **Copy row** copies its cells as shown, separated by tabs (they paste into a
  spreadsheet as one row), and **Copy k-mer** a variant's k-mer.
- Desktop app: notifications stack upwards from 16 px above the window's bottom-right corner,
  the newest nearest the corner, each as wide as its message and buttons (at most 380 px). A
  message wraps beside its buttons, which sit at its top right (× and Copy details), and screen
  readers read the message before them.
- Desktop app: **Help ▸ Keyboard shortcuts** also lists Enter (press or select; in a filter field,
  apply the value typed), Esc in a filter field and the right click that copies a table row.

### Fixed

- The end-of-run line showed a positive mean or minimum below 0.00005 bits as `0.0000`; it now
  reads `< 0.0001`, as in the desktop app.
- The hint of the `--hcs-min-incidence`/`--hcs-eligible-only` warning suggested `--table hcs`
  alone, which also needs `-o PATH.tsv`; it now reads "add --hcs-output PATH or --tsv-dir DIR,
  or write the table with -o PATH.tsv --table hcs".
- The counter-overflow message said "64-bit" for counters that are 32-bit (distinct k-mers at
  one position, distinct header values); it now reads "… exceeds the range of DiMA's counters".
- `dima view`, `dima hcs` and `dima cite --from` checked the ranges of every value of a position
  but not of the summaries, so a results file edited to hold, for example, a negative mean or a
  coverage above 1 was read and written on. Such a file is now refused as edited or damaged
  (exit 4), like one with an impossible value at a position.
- Messages about the schema version of a conserved-sequences file no longer call it a results
  schema, and the hint for one written by a newer DiMA names its `tool.version` (results files
  have `run.tool.version`).
- The desktop app's entropy chart could keep drawing the solid line of the previous results
  when new results showed the same positions, zoom and highest entropy (for example the same
  alignment analysed again with the raw estimator and another support threshold); the line is
  now redrawn for every new set of results.
- The desktop app's entropy axis repeated its labels (0.00, 0.01, 0.01, 0.02) when the highest
  shown entropy was below about 0.024 bits, on screen and in exported figures. Ticks less than
  0.01 bits apart now get three decimals, and the axis is at least 0.005 bits tall (1 bit, as
  before, when every shown value is 0).
- Sorting the desktop app's position table by Top variant copied both k-mers in every
  comparison; it now compares them in place, in the same order and more than twice as fast
  (half a million positions: 0.3–0.4 s instead of 0.8–0.9 s).
- The desktop app reduced the chart's line and scanned every shown position on every frame
  it drew; the line is now computed once per change of the shown positions, zoom or chart
  width, so drawing a frame no longer goes through every shown position (half a million
  positions: 0.09 ms instead of 5.2 ms per frame). As in 1.0.0, each frame still goes through
  every separate piece of the line, every no-support band and every coverage mark, so a chart
  with many of them still takes longer to draw.
- The desktop app's exported PNG figures drew their text about a quarter smaller than the SVG
  and the screen, and their title in the regular weight; the legend was spaced for that
  smaller text, so in SVG figures its labels ran into the next entry. Text is now sized alike
  in both formats, with a SemiBold title. Exported figures are at least 720 × 320 px and at most
  20,000 × 20,000 px (a saved size outside that range is moved into it once, with a note, where
  a size above the maximum used to reset every setting), the legend continues on further rows when it is
  wider than the figure, and a title, subtitle or legend label too long for the figure is
  shortened with "…".
- The desktop app showed a positive value too small for its decimals as `0.0000` (or `0.0%`),
  and a coverage, incidence, conservation bound or share of a variant's sequences just below
  its maximum as `1.0000` (or `100.0%`). Tables (the metadata table included), panels,
  tooltips, chips and exported figures now read less or more than the smallest step they
  show, for example `< 0.0001`, `< 0.01%`, `> 0.9999` and `> 99.9%`; sorting and exports keep
  full precision.
- In a narrow desktop-app window (980 px, the smallest allowed) the results header broke its
  fact chips into columns a few characters wide, hundreds of pixels tall, which pushed the
  chart and the position table out of the window. Each chip now stays on one line and the row
  wraps between chips. When the chart, the conserved regions and the table still do not all fit,
  their column scrolls (the mouse wheel over the chart keeps zooming it), and the table keeps at
  least four rows.
- The desktop app's chart drew a position that has support but no entropy (possible only in a
  results file edited by hand) as a hatched band labelled "no usable support". The band now
  marks positions with support 0 only; a missing value just breaks the line.
- The desktop app's position facts cut a long value, such as a method with its fallback
  ("plug_in; fallback: support at or below threshold"), at the panel's edge; it now wraps.
- In narrow desktop-app windows the filter bar's position count was drawn over the low-support
  switch, and the chart's "drag: pan" hint over its legend. Both now move to a row of their own
  when their row is full (for the count and Reset, when it could not hold the longest count, so
  a count that changes with a filter or a metric switch never moves them to another row), and
  the chart's Fit, + and − buttons stay at the top right of the chart, where screen readers meet
  them after the chart's heading and legend, as in 1.0.0.
- Moving the keyboard focus through a value field of the desktop app's filter bar, with Tab for
  example, stored the value as the field showed it, rounded. When the field rounded the largest
  value down, the range then hid the positions above the rounded value, the one with the largest
  value among them, and turned Reset on, although nothing had been changed. A minimum that reads
  as 0 is now 0, and a maximum that reads as the largest value is that value exactly.
- The command line's `--table` help and the desktop app's tooltip of the TSV-folder export said
  that `NA` marks undefined values; it also marks values that were not computed, such as the
  similarity-weighted columns without the option, and both now say so.
- The methods text (`dima cite --from`, and **Methods & references** in the desktop app) of a run
  with one position read "the overlapping 120-mers at each of the 1 position were counted"; it
  now reads "at its one position".
- Desktop app: a figure export did nothing, and said nothing, when the filters hid every position
  while its save dialog was open; a warning now says that nothing was saved and why.
- Desktop app: the filter bar's fields and the variant table's minimum incidence had no
  accessible name, so a screen reader announced only "max 1.5878", "from 1" or "0.0%"; each now
  has one that says what it bounds ("First position shown", "Last position shown", "Minimum
  incidence", and for the value fields the bound and the metric, for example "Highest entropy
  shown, in bits"), which is also the filter bar fields' tooltip.
- Desktop app: a value typed into a value field with more decimals than the field shows was kept
  whole, and the field read its rounded text back the next time the keyboard focus passed
  through it: a minimum typed as 0.111293 (shown as 0.1113) became 0.1113, hid position 1 of the
  MERS spike sample (entropy 0.111294) and reset the zoom, with nothing edited. A typed value is
  now kept to the four decimals the field shows.
- Desktop app: switching the chart's metric in the same frame as a filter edit (a field left by
  clicking the switch) applied the edit again on the next frame and lost the zoom, although the
  switch keeps it. The switch now applies the edit itself.
- Desktop app: a notification of three lines or more grew past the bottom of the window, its
  close button half off it (for example the warning "Nothing was saved: the chart was switched
  to …"), and the text of a wrapped notification was right-aligned with its buttons on its left.
- Desktop app: the filter bar's position fields moved every control after them when one was
  focused (it then shows its number without "from"/"to") or showed a shorter number ("to 500");
  they now keep their width.
- Desktop app: results written with a newer schema (or one this version does not recognise)
  opened without a word, and an export from them left out the fields this version does not
  know. The app now shows the warning `dima view` gives: "The results were written with schema
  1.2.0; this version writes 1.1.0 and keeps only the fields it knows. Use the DiMA version that
  wrote the file to keep every field." (an unrecognised version is quoted).
- Desktop app: the dot that marks an option changed from its default moved every control of
  its grid by 12 px when it appeared on the widest option name (ticking **Compute
  similarity-weighted entropy** or changing T), so a second click at the same place missed. Its
  place is now kept free.
- Desktop app: the end of a long fact of the selected position (a method with its fallback) ran
  under the panel's scroll bar; the facts now wrap before it.
- Desktop app: the explanations of the results header's chips were tooltips only; screen readers
  now read them too, as the facts' explanations.
- Desktop app: the close buttons of notifications and of the recent-files list showed an empty
  box (✕ is in none of the app's fonts); they show × and are named "Dismiss" and "Remove <file>
  from recent files" for screen readers. The ▸ of "Export ▸ Methods & references" in the Cite window
  and the ▼ that marks a coverage below the minimum in the position table also showed as empty
  boxes since 1.0.0; they are now drawn.
- Desktop app: the Setup screen's fields, menus and sliders (alphabet, k, T, the query name, the
  conserved-region threshold, the Advanced options, the path field and the header-format editor),
  the Preferences' figure size and the command palette had no accessible name, so a screen
  reader announced only their values ("9", "100"); each is now named by the label beside it or
  by what it is for (since 1.0.0).
- Documentation: the precision of large runs was given as measured at 2,208,211 and 17,500,000
  sequences per position, but the rounding error changes from one number of sequences to the
  next. "Numerical accuracy" now gives the largest error, which follows from rounding the most
  common k-mer's frequency: about 8e-12 relative at 2.2 million sequences and 6e-11 at 17.5 million
  (it said 2.2e-12 and 5e-11), still at least 10^10 times below the sampling error.
- Documentation: the numbers that came from random runs and simulations are now exact wherever
  an exact form exists, and seeded and stated with their precision otherwise. Among the changes:
  the paper's 1.38 bits at MERS-CoV spike position 25 lies 0.29 standard deviations below the
  exact expectation (it said "about its 39th percentile"); the mean of one run of the random
  procedure on HIV-1 *pol* has a standard deviation of 0.0020 bits (it said "repeated runs give
  5.350–5.353"); DiMA v5.0.9's rule raises the average by 50% at N = 171 and 105% at N = 130 (it
  said 51% and 107%); its MERS-CoV spike mean is 0.1417 bits (it said "about 0.142"), and on
  HIV-1 *pol* its procedure, applied to the same k-mers, changes the means by less than
  0.00026 bits (DNA) and 0.0011 bits (protein), its counting of stop and frameshift symbols
  changing the protein mean more (it said "the two agree"); in the population study the exact expectation underestimates by up
  to 6.4% when at least one minor-variant sequence is expected (it said "at most 3%"), and
  v5.0.9's rule overestimates by up to 31% with T = 100 and 20% with T = 30 (it said 19%).
- Documentation: some rows of "Design rationale" cited sources for more than those sources say;
  each now cites the rule, section or table that supports it and states the rest in plain words.
- Documentation: the methodology page now cites the classical results behind its exact standard
  deviations and its bounds on DiMA v5.0.9's HIV-1 *pol* means (Cantelli's, Scarf's and
  McDiarmid's inequalities, and the Krawtchouk expansion and recurrence), and the v5.0.9
  comparison says that it applies v5.0.9's procedure to the same k-mers.
- Documentation: the desktop-app guide said that the position table can be sorted by any column
  and that the similarity-weighted column is sortable like the others; the Coverage column
  cannot be sorted, and the guide now says so.
- Documentation: "Large alignments" gave the memory of the results as a few hundred bytes per
  position; it now also counts the variants they hold until they are written (each its k-mer and
  about 72 bytes), which can be most of it on diverse alignments. The exit-code reference now
  lists edited or damaged results files among the causes of exit status 4, and Troubleshooting
  gives the message an unrecognised schema version stops with.

### Security

- Dependencies updated for RustSec advisories, none of which affects how DiMA uses them: zstd
  0.13.4, zstd-safe 7.4.0 and zstd-sys 2.1.3 (RUSTSEC-2026-0391, -0398, -0403 and -0410:
  context reuse, seekable archives, legacy frames and WebAssembly, none of which DiMA uses; the
  bundled zstd library stays 1.5.7, so compressed outputs are unchanged), and, in the desktop
  app, url 2.5.9 (RUSTSEC-2026-0592) and the yanked async-executor 1.14.0 and clipboard-win
  5.4.1. The zstd crates are now BSD-3-Clause licensed (THIRD-PARTY-NOTICES.md).

## [1.0.0] - 2026-10-07

First release of the Rust implementation of DiMA (Tharanga et al. 2025, *Briefings in
Bioinformatics* 26(1):bbae607, [doi:10.1093/bib/bbae607](https://doi.org/10.1093/bib/bbae607)).
It replaces every earlier version of this repository; results of those versions are not
comparable (see "Fixed").

### Methodology

- **Entropy correction (`--estimator dima`, the default) follows the paper's Fig. 2 exactly**:
  subsample sizes of R% of the sequences, with R from max(50, ⌈100·T/N⌉) rounded up to a
  multiple of 5 in steps of 5, plus 100%; sizes ⌈R·N/100⌉; sampling with replacement; the
  entropy regressed on 1/n; the intercept is the estimate, with the paper's fallbacks (raw
  entropy when N ≤ T or with fewer than 5 points). It is computed as the
  exact expectation of the random intercept, so results are reproducible bit for bit, with the
  random intercept's mean and no resampling noise (Rao 1945; Blackwell 1947,
  [doi:10.1214/aoms/1177730497](https://doi.org/10.1214/aoms/1177730497)): the limit of
  averaging many random subsets, as DiMA's predecessors did (Khan et al. 2017,
  [doi:10.1186/s12920-017-0301-2](https://doi.org/10.1186/s12920-017-0301-2)). The expected
  intercept is never negative, so the paper's negative-intercept rule never applies; DiMA
  v5.0.9 applied it to each random run, which raised its average at near-conserved positions
  (MERS-CoV spike protein eligible mean 0.126 bits here, about 0.142 averaged over v5.0.9 runs;
  HIV-1 pol unchanged). Every reported entropy is checked to be a finite number of at least 0
  bits; a violation is an internal error (exit 1), never replaced by another value. On the paper's HIV-1 pol dataset the eligible mean is 5.3504 bits (paper: ~5.35).
- **Alternative estimators**: `chao-wang-jost` (Chao, Wang & Jost 2013,
  [doi:10.1111/2041-210X.12108](https://doi.org/10.1111/2041-210X.12108)), `miller-madow`
  (Miller 1955) and `raw` (plug-in). Each position records the estimator or fallback used.
  The second term of Chao–Wang–Jost is evaluated exactly in a time independent of the number of
  sequences (hypergeometric series or Gauss's continued fraction: Hankin 2015,
  [doi:10.32614/RJ-2015-022](https://doi.org/10.32614/RJ-2015-022); Van Vleck 1901,
  [doi:10.2307/1967627](https://doi.org/10.2307/1967627); Henrici & Pflüger 1966,
  [doi:10.1007/BF02166031](https://doi.org/10.1007/BF02166031); Jones & Thron 1974,
  [doi:10.1090/S0025-5718-1974-0373265-5](https://doi.org/10.1090/S0025-5718-1974-0373265-5);
  Alzer 1997, [doi:10.1090/S0025-5718-97-00807-7](https://doi.org/10.1090/S0025-5718-97-00807-7)),
  where summing it over every sequence would cost about 10% of a run and lose up to 8 digits.
- **Sample coverage** per position (Chao & Jost 2012, Eq. 4a,
  [doi:10.1890/11-1952.1](https://doi.org/10.1890/11-1952.1)), with an optional
  `--min-coverage` flag.
- **Positions without support** (NS) have no entropy (`null`/`NA`), never 0 bits; summaries
  cover positions with support above T only, with the paper's averaging also reported as
  `all_positions_summary`.
- **Lowest entropy** is reported next to the mean and the highest, each with its position and
  the number of tied positions (first position wins).
- **Historically conserved sequences** follow the DiMA lineage (Hu et al. 2013,
  [doi:10.1371/journal.pone.0059994](https://doi.org/10.1371/journal.pone.0059994); Abd Raman
  et al. 2020, [doi:10.7717/peerj.7954](https://doi.org/10.7717/peerj.7954)): positions whose
  Index incidence reaches the threshold (above 50% and at most 100%, compared exactly) are
  joined when their k-mers overlap or abut and agree; regions that include positions with
  support ≤ T are flagged, and each region reports the exact 95% Clopper–Pearson lower bound
  of its conservation (Clopper & Pearson 1934,
  [doi:10.1093/biomet/26.4.404](https://doi.org/10.1093/biomet/26.4.404)).
  `--hcs-eligible-only` builds regions from positions with support above T only.
- **Alphabet**: detected with Easel's rules as in HMMER (Eddy 2011,
  [doi:10.1371/journal.pcbi.1002195](https://doi.org/10.1371/journal.pcbi.1002195)) or set
  with `--alphabet`; lower case is folded to upper case; T and U are one symbol; gaps
  (`- . _`), missing data (`~ ?`), stops (`* $ ^`), frameshifts (`#`) and ambiguity codes
  (Cornish-Bowden 1985, [doi:10.1093/nar/13.9.3021](https://doi.org/10.1093/nar/13.9.3021);
  Los Alamos HIV Sequence Database symbols) exclude the k-mers that contain them; protein U and
  O are residues.
- **Default k** follows the alphabet: 9 for proteins, 27 for nucleotides. Any k from 1 to the
  alignment length is accepted.
- **Header metadata**: values are split on `|`, cleaned (invisible characters removed, white
  space trimmed, Unicode NFC; Unicode Standard Annex #15) and empty values get the fill value.

### Added

- `dima analyze`, `view`, `hcs`, `cite` and `completions` subcommands; `--run-report` writes
  a separate JSON record of how a run went (versions, command, times, threads, resources,
  checksums) without identifying details.
- Outputs: complete JSON with provenance, JSONL (one position per line), four tidy TSV tables
  (positions, variants, metadata, conserved sequences) and conserved-sequence JSON, each
  optionally compressed (`.gz`, `.zst`); JSON Schemas (Draft 2020-12) for every JSON output.
- Typed diagnostics in the results and on standard error, each with a count, examples and a
  hint; `--validation strict|permissive|report`.
- Input: plain, gzip/BGZF, bzip2 and zstd files (xz and other formats are refused with a hint
  on how to convert them), concatenated streams, standard input, pipes and FIFOs; the SHA-256
  of the stored input is recorded.
- `dima cite` prints the references in text, BibTeX, RIS or CFF; `--from results.json`
  prints a methods paragraph and the references of that run. `CITATION.cff` and
  `.zenodo.json` describe the software.
- Interruptions: SIGINT, SIGTERM and SIGHUP (and SIGUSR1, SIGUSR2, SIGXCPU and SIGALRM, which
  batch systems send before a limit) stop the run, remove unfinished outputs and end it by the
  same signal; outputs are written to temporary files and replace their destinations only when
  complete, keeping the permissions of a replaced file.
- Batch-scheduler-aware default thread count (Slurm, PBS, SGE, LSF). When the system refuses
  some worker threads (a process, thread or container limit), the analysis continues with those
  it started and warns (`threads_refused`); the run report records both numbers.
- Messages, `--help` (apart from authors' names) and the progress line are plain ASCII, so they
  read the same in any locale, pager, Windows console or screen reader;
  `--progress auto|plain|off` (or `DIMA_PROGRESS`) chooses between a line redrawn on terminals,
  appended lines for screen readers and logs, and nothing; colour follows `--color`,
  `NO_COLOR`, `FORCE_COLOR` and `CLICOLOR_FORCE`.
- Numerical accuracy is measured and documented up to 17.5 million sequences per position:
  rounding error reaches at most 5e-11 relative, at least 10^10 times below the sampling error
  (Darriba et al. 2018,
  [doi:10.1093/molbev/msy014](https://doi.org/10.1093/molbev/msy014)). A fix for the last
  digits was prototyped and is deferred, because it would change stored digits without
  changing any scientific result.
- Desktop application on the same library.

### Changed

- New command-line names (`-s/--support-threshold`, `-a/--alphabet`, `--estimator`,
  `--query-name`, `--hcs-min-incidence`, …); the support threshold defaults to 100 as in the
  paper.
- Exit codes: 0 success, 1 internal error, 2 usage, 3 input/output or system error (such as a
  refused thread), 4 invalid input data, 130/143/129 interrupted.
- Results are identical across operating systems, thread counts and batch sizes, and for
  wrapped, lower-case, CRLF or compressed copies of the same alignment.
- Lines before the first record that are blank or hold only white space are skipped (DiMA
  v5.0.9 stopped at any line before the first header); other text before it, including an
  indented `>`, is an error with its line and column.
- Memory grows with the diversity of the alignment, not with the number of sequences times
  its length.

### Removed

- The binary `.dima` format, memory-mapping and disk-backed matrix options
  (`--low-memory`, `--force-ram`, `--temp-dir`), `--allow-lowercase`, `--report-invalid`,
  `--no-header` and the record-length and sequence-count limits.

### Fixed

- Line-wrapped FASTA files were read with line breaks inside k-mers.
- Lower-case residues were rejected, and malformed records were skipped silently.
- Concatenated bzip2 streams were truncated after the first stream.
- The equal-length check compared k-mer counts instead of alignment columns.
- k was limited to 14 (protein) or 27 (nucleotide).
- Several options had no effect; the desktop app and the command line disagreed on defaults
  and alphabets.
