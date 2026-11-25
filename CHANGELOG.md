# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). The
"Methodology" group lists every change that can alter a result, with the sources behind it.

## [1.1.0] - Unreleased

### Methodology

- **Similarity-weighted entropy (opt-in)**: Wu J. 2025, "Similarity-weighted entropy for
  quantifying genetic diversity in viral quasispecies", *Virus Evolution* 11(1):veaf029
  ([doi:10.1093/ve/veaf029](https://doi.org/10.1093/ve/veaf029)), Section 2.1 and
  Supplementary File 1. Each variant's entropy term is weighted by its dissimilarity to the rest
  of the position, the similarity of two k-mers being their proportion of identical residues
  (Grishin 1995, [doi:10.1007/BF00175826](https://doi.org/10.1007/BF00175826), p. 675). Also the
  observed and similarity-weighted entropies normalised by log2 of the number of distinct
  k-mers (the Index included); undefined with one distinct k-mer. Computed exactly from residue
  counts per site instead of pairs of variants (the column-profile form of sum-of-pairs scores,
  Gotoh 1994, [doi:10.1093/bioinformatics/10.4.379](https://doi.org/10.1093/bioinformatics/10.4.379),
  eqs. (1), (4a), (9b)). Without the option nothing is computed and no existing value changes;
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
  Without the option the keys are `null` (`NA` in the table); default results grow by 116 bytes
  per position (compact JSON and JSONL; 140 indented; 9 per positions-table row).
- Desktop app: **Compute similarity-weighted entropy (Wu 2025)** in the Advanced options, off by
  default, remembered between sessions and turned off by **Reset to defaults**. The closed
  Advanced section's title counts the options inside it that differ from their defaults
  ("Advanced (1 changed)"), and **Run details** says whether similarity-weighted entropy was
  computed, for analyses run in the app and for results opened from a file. The new setting is
  saved only when it is on, so default settings stay readable by dima 1.0.0.
- Desktop app: when the run computed similarity-weighted entropy, a **Metric:** switch above the
  chart plots **Entropy** or **Similarity-weighted entropy**. Each has its own value axis,
  heading, legend and mean line, and the filter bar's value range applies to the metric shown. A
  switch keeps the zoom and the selection and resets the value range (with a note when it had
  been narrowed); no control moves. Every dataset opens showing entropy. Exported figures show
  the metric on screen, with its title, axis title, legend and mean.
- Desktop app, when the run computed similarity-weighted entropy: a chip "similarity-weighted
  mean X bits (observed Y)" after the mean chip (0.0363 and 0.1207 bits for the bundled MERS
  spike sample); a sortable **Similarity-weighted** column right after **Entropy** in the
  position table; and the selected position's similarity-weighted and normalised entropies in
  its facts and in the chart's tooltip, the normalised values marked "(low support)" at support
  ≤ T and reading "— (one variant)" where a single k-mer leaves them undefined.
- Desktop app: the position table scrolls sideways when its columns do not fit the window, and
  every sortable header has its "Sort by …" tooltip as its accessibility description.

### Changed

- Results schema 1.1.0 (the conserved-sequences schema stays 1.0.0 and now has its own version
  constant). Results written by dima 1.0.0 are read as runs without similarity-weighted entropy;
  `dima view` notes this when it writes values their schema did not record, and keeps its
  `newer_schema` warning for newer or unrecognised versions.

### Fixed

- The end-of-run line showed a positive mean or minimum below 0.00005 bits as `0.0000`; it now
  reads `< 0.0001`, as in the desktop app.
- The hint of the `--hcs-min-incidence`/`--hcs-eligible-only` warning suggested `--table hcs`
  alone, which also needs `-o PATH.tsv`; it now reads "add --hcs-output PATH or --tsv-dir DIR,
  or write the table with -o PATH.tsv --table hcs".
- The counter-overflow message said "64-bit" for counters that are 32-bit (distinct k-mers at
  one position, distinct header values); it now reads "… exceeds the range of DiMA's counters".
- Messages about the schema version of a conserved-sequences file no longer call it a results
  schema.
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
  width, so a frame's cost no longer grows with the number of positions (half a million
  positions: 0.09 ms instead of 5.2 ms per frame).
- The desktop app's exported PNG figures drew their text about a quarter smaller than the SVG
  and the screen, and their title in the regular weight; the legend was spaced for that
  smaller text, so in SVG figures its labels ran into the next entry. Text is now sized alike
  in both formats, with a SemiBold title. Exported figures are at least 720 × 320 px (a smaller
  saved size is raised once, with a note), the legend continues on further rows when it is
  wider than the figure, and a title, subtitle or legend label too long for the figure is
  shortened with "…".
- The desktop app showed a positive value too small for its decimals as `0.0000` (or `0.0%`),
  and a coverage, incidence or conservation bound just below its maximum as `1.0000` (or
  `100.0%`). Tables, panels, tooltips, chips and exported figures now read `< 0.0001`
  (`< 0.1%`) and `> 0.9999` (`> 99.9%`); sorting and exports keep full precision.
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
  when their row is full, and the chart's Fit, + and − buttons stay at the top right of the
  chart.

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
