---
title: Analyse large alignments
description: Analyse alignments of hundreds of thousands to millions of sequences — compression formats, memory use, metadata, output size and how to find the bottleneck.
---

DiMA reads an alignment once, as a stream, and never holds all of it in memory: alignments of
millions of sequences can be analysed on an ordinary workstation. This guide explains what
determines the time and memory a run needs.

## Compress the input with Zstandard

DiMA decompresses gzip, bzip2 and Zstandard itself, so there is no need to store large
alignments uncompressed. Compressors differ widely in how fast they decompress sequence data
[@kryukov2020], so the format affects the speed of reading:

- **Zstandard** (`.zst`) decompresses fastest and compresses alignments well, especially with a
  long matching window; DiMA reads windows up to 2 GiB (`--long=31`):

  ```sh
  zstd -T0 -19 --long=31 alignment.fasta -o alignment.fasta.zst
  ```

- **gzip** (including BGZF) is the most widely supported and reasonably fast to decompress.
- **bzip2** compresses well but decompresses slowly.
- **xz** is not read: decompress it first, or recompress it with Zstandard once, which keeps it
  as small:

  ```sh
  xz -dc alignment.fasta.xz | zstd -T0 -19 --long=31 -o alignment.fasta.zst
  ```

For the paper's HIV-1 *pol* DNA alignment (9.2 MB), `gzip -9` gives 741 KB, `bzip2 -9` 508 KB,
`zstd -19 --long=31` 423 KB (and `xz -9` 462 KB). For alignments of many closely related genomes
the difference grows: Zstandard's long window finds the repeats between sequences that lie far
apart, while gzip only looks back 32 KB.

With `-v`, DiMA reports the time spent in each phase and says so when reading the input limited
the run; compressing with Zstandard is then the remedy ([Read the -v report](#read-the--v-report)).

## Read the -v report

`-v` adds a short report on standard error: what the run used and where its time went. For
example, 26,370 HIV-1 *pol* DNA sequences compressed with bzip2: the repository's HIV-1 *pol*
DNA alignment with ten mutated copies of each sequence, made in a clone of the repository with

```sh
bzip2 -dc tests/data/HIV1_FLT_2022_pol_DNA.fasta.bz2 > hiv.fasta
python3 scripts/bench/make_data.py scale hiv.fasta 10 hivx10.fasta
bzip2 -1 hivx10.fasta
dima analyze -i hivx10.fasta.bz2 -o x10.json -v
```

```text
threads: 4 (available CPUs; OS 4)
[0.00 s] opening
[0.68 s] counting k-mers
[1.77 s] computing statistics
[2.83 s] writing
wrote x10.json (278 MB)
phases: opening 0.68 s, counting k-mers 1.08 s, computing statistics 1.07 s, writing 1.28 s
reading the input limited the run (waited 0.86 s of 1.08 s); zstd decompresses fastest
CPU 9.09 s user + 0.29 s system, peak memory 246 MB
analysed 26370 sequences, 3364 positions (2159 eligible) in 4.11 s; entropy mean 5.6032, min 1.3112, max 9.5310 bits
```

Line by line:

| Line | Meaning |
|---|---|
| `threads: 4 (available CPUs; OS 4)` | The number of worker threads, and where it came from: `--threads`, `RAYON_NUM_THREADS`, a batch scheduler's variable (such as `SLURM_CPUS_ON_NODE=8`, or `PE_HOSTFILE[node12]=8`, which names this host), or `available CPUs`, the CPUs this process may use. `OS 4` is that number of CPUs. A note follows when `OMP_NUM_THREADS` is lower: DiMA does not use it ([Run on clusters](/how-to/hpc/#threads)) |
| `[0.68 s] counting k-mers` | A phase began, at that time since the start. These lines appear with `-v` when standard error is not a terminal (a log file, a pipe); on a terminal, one line redrawn in place shows the progress instead, as below |
| `wrote x10.json (278 MB)` | A file written, with its size (MB is 10⁶ bytes) |
| `phases: …` | How long each phase took. **opening**: opening the input, reading its first batch (up to 10,000 records or 64 MiB, and at least 1,000 records) and detecting the alphabet; **counting k-mers**: reading the rest and counting the k-mers; **computing statistics**: the entropy, motifs and incidences of every position; **writing**: computing the conserved sequences, when an output holds them, and writing every output |
| `reading the input limited the run (waited … of …)` | Only when counting took more than half a second and the threads that count spent more than half of it waiting for the input: here 0.86 of 1.08 seconds. Reading, not computing, set the pace: recompress the input with Zstandard, or read it from a faster disk |
| `CPU 9.09 s user + 0.29 s system, peak memory 246 MB` | The CPU time of all threads together (it can exceed the run time) and the peak resident memory, the most the process held at once (`unknown` where the system does not tell) |
| `analysed …` | The [summary line](/reference/glossary/#summary-line), always last |

The [run report](/how-to/reproducibility/#keep-a-run-report) (`--run-report`) records the same
figures: its phases `opening`, `counting`, `finalising` and `writing` are the phases above (the
last two are `computing statistics` and `writing`), `threads.source` names where the thread count
came from (`available_parallelism` is `available CPUs`), and `resources` holds the CPU time, the
peak memory and the time spent waiting for the input.

On a terminal, without `-v`, the progress is one line redrawn in place, for example:

```text
counting k-mers: 96%, 10000 sequences, 26.2 MB of 27.3 MB
computing statistics: 52%, 1769 of 3364 positions
writing...
```

While counting, the percentage is that of the file's bytes as stored (compressed, for a
compressed file) and stays at 99% until counting ends; the number of sequences grows a batch at
a time, up to 10,000 sequences per batch. From standard input there is no total, so the line
shows how much has been read: `counting k-mers... 5.6 MB`. A phase without a percentage shows
how long it has lasted after 2 seconds (`writing... 3 s`). `--progress plain` prints appended
lines instead, for logs and screen readers; see
[Environment variables](/reference/environment/#progress-the-order).

## Memory

DiMA's memory does not grow with the number of sequences times their length. It holds:

- at most two batches of input, each up to 10,000 records or 64 MiB — except the first, which
  holds at least 1,000 records (or all of them) to detect the alphabet, so with very long
  sequences it can be larger;
- for each position, its distinct k-mers — about $k + 25$ bytes each — and, with metadata, about
  19 bytes for each combination of k-mer and header value;
- with `--similarity-weighted-entropy`, for each position being computed (so one per thread at
  a time): a table of $26 \times \min(k, 2048)$ counters, $208 \cdot k$ bytes up to $k = 2{,}048$
  and 426 kB above it (5.6 kB at $k = 27$, 208 kB at $k = 1{,}000$), and about 40 to 80 bytes
  per distinct k-mer at the position (the upper end when most k-mers have different counts);
- the results until they are written: about 180 bytes per position (48 of them, since dima 1.1,
  for the three similarity-weighted values, whether or not they are computed), plus, for every
  variant, its k-mer and about 72 bytes, and its metadata counts when metadata are used.

Memory therefore grows with the **diversity** of the alignment: conserved alignments need little
even with millions of sequences, highly diverse ones more. The number of threads does not change
it. `-v` prints the peak memory, and `--run-report` records it.

To size a job, measure: run the analysis once with `-v` (or `--run-report`) on the full
alignment, or on the largest you have of the same kind, and request its peak memory with a
margin. [BENCHMARK.md](https://github.com/ShanWeera/dima-perf/blob/master/BENCHMARK.md) gives
peaks for the repository's datasets, measured with dima 1.1.0 in its quick mode (3 runs per
measurement, synthetic data scaled up only 10 times, on a 4-CPU virtual machine): 8 MB for the
MERS-CoV spike protein (191 sequences), 20 MB for HIV-1 *pol* protein (3,756), 111 MB for HIV-1
*pol* DNA (2,637), 253 MB for ten times that (26,370), 183 MB for 200 synthetic 30 kb genomes,
and 731 MB for the same genomes stratified by two header fields. Larger and more diverse
alignments need more; the full benchmark (`scripts/bench/run.sh` without `--quick`, 2.6 million
sequences) measures them on your hardware.

## Metadata

Header metadata multiplies the size of the results: every variant stores one count per value of
every [stratified field](/reference/glossary/#stratified-field). Stratify only by the fields you
need, and never by an identifier such as an accession, which adds one entry per sequence to
every variant. DiMA warns when a stratified field looks like an identifier and prints the option
that leaves it out; see [Stratify by header metadata](/how-to/metadata/#identifier-like-fields).

## Similarity-weighted entropy

`--similarity-weighted-entropy`
([what it computes](/explanation/entropy/#similarity-weighted-entropy-opt-in)) adds work at
every position with support. There, its time is proportional to the number of distinct k-mers
times $k$: DiMA counts the residues at each of the $k$ offsets of the k-mers once, instead of
comparing every pair of k-mers. Its memory is listed under [Memory](#memory): a table of at most
426 kB and about 40 to 80 bytes per distinct k-mer, for each position being computed. Above 2,048 sites
the table is filled 2,048 sites at a time, so that it stays in the processor's cache. It is
computed only when an output holds its values (the results, or the positions table).

Measured on 25 November 2026 with a release build, on a virtual machine with 4 CPUs (Intel Xeon
at 2.30 GHz) and 16 GB of memory, running Ubuntu 24.04 and other work at the same time. Each
time is the median of 11 to 31 runs, alternating with and without the option, using all 4
threads; CPU time is the sum over the threads:

| Alignment | $k$ | Distinct k-mers per position with support (mean) | Without the option | With it |
|---|---|---|---|---|
| HIV-1 *pol* DNA (`tests/data/HIV1_FLT_2022_pol_DNA.fasta.bz2`, 2,637 sequences) | 27 | 293 | 1.05 s (CPU 1.58 s) | 1.06 s (CPU 1.64 s) |
| The same, ten mutated copies of each sequence (26,370 sequences, from `scripts/bench/make_data.py`) | 27 | 725 | 2.53 s (CPU 6.11 s) | 2.54 s (CPU 6.17 s) |
| MERS-CoV spike gene (`tests/data/MERS_Spike_nt.fasta.bz2`, 499 sequences), results discarded with `-o /dev/null -O json` | 1,000 | 123 | 0.74 s (CPU 1.24 s) | 0.93 s (CPU 1.88 s) |

At $k = 27$ the option added about 0.05 s of CPU time and almost nothing to the wall time. Peak
memory did not change measurably: about 120 MB, 265 MB and 630 MB with or without it. With long
k-mers its share grows, because every distinct k-mer costs $k$ steps: at $k = 1{,}000$ it
increased the CPU time by about half and the wall time by about a quarter. Long k-mers also make
most windows distinct, so the counting itself then needs much more memory (see
[Memory](#memory)); the option adds only its table and its 40 to 80 bytes per distinct k-mer.

## Output size

The results list every variant of every position. Write them compressed and convert later as
needed:

```sh
dima analyze -i big.fasta.zst -o big.json.zst
dima view -i big.json.zst --tsv-dir tables          # tables only when you need them
```

JSON Lines (`-o big.jsonl.zst`) holds one position per line, so other tools can process it
position by position without loading the whole file.

Each output is written to a temporary file next to its destination and then renamed, so the
destination's file system needs room for the new file while an old one still exists.

## Threads

DiMA splits the positions among threads; the counting scales with the number of CPUs until
reading the input becomes the bottleneck. See
[Run on clusters and in pipelines](/how-to/hpc/#threads) for the default on shared machines.

## A checklist

1. Compress the alignment with Zstandard.
2. Count metadata only for the fields you need.
3. Write compressed results (`.json.zst`).
4. Run with `-v` once to see where the time goes and how much memory the run needs, or keep a
   `--run-report`.

## References

[^ref]
