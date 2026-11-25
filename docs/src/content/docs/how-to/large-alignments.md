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
the run; compressing with Zstandard is then the remedy.

## Memory

DiMA's memory does not grow with the number of sequences times their length. It holds:

- at most two batches of input, each up to 10,000 records or 64 MiB — except the first, which
  holds at least 1,000 records (or all of them) to detect the alphabet, so with very long
  sequences it can be larger;
- for each position, its distinct k-mers — about $k + 25$ bytes each — and, with metadata, about
  19 bytes for each combination of k-mer and header value;
- with `--similarity-weighted-entropy`, a table of $26 \times k$ counters ($208 \cdot k$ bytes)
  for each position being computed, so one per thread at a time: 5.6 kB per thread at $k = 27$,
  208 kB at $k = 1{,}000$;
- the results until they are written.

Memory therefore grows with the **diversity** of the alignment: conserved alignments need little
even with millions of sequences, highly diverse ones more. The number of threads does not change
it. `-v` prints the peak memory, and `--run-report` records it.

## Metadata

Header metadata multiplies the size of the results: every variant stores one count per value of
every counted field. Count only the fields you need, and never an identifier such as an
accession, which adds one entry per sequence to every variant. DiMA warns when a counted field
looks like an identifier and prints the option that leaves it out; see
[Stratify by header metadata](/how-to/metadata/#identifier-like-fields).

## Similarity-weighted entropy

`--similarity-weighted-entropy`
([what it computes](/explanation/entropy/#similarity-weighted-entropy-opt-in)) adds work at
every position with support. There, its time is proportional to the number of distinct k-mers
times $k$: DiMA counts the residues at each of the $k$ offsets of the k-mers once, instead of
comparing every pair of k-mers. Its memory is the small table listed under [Memory](#memory).
It is computed only when an output holds its values (the results, or the positions table).

Measured on 25 November 2026 with a release build, on a virtual machine with 4 CPUs (Intel Xeon
at 2.30 GHz) and 16 GB of memory, running Ubuntu 24.04 and other work at the same time. Each
time is the median of 11 to 31 runs, alternating with and without the option, using all 4
threads; CPU time is the sum over the threads:

| Alignment | $k$ | Distinct k-mers per position (mean) | Without the option | With it |
|---|---|---|---|---|
| HIV-1 *pol* DNA (`tests/data/HIV1_FLT_2022_pol_DNA.fasta.bz2`, 2,637 sequences) | 27 | 234 | 1.05 s (CPU 1.58 s) | 1.06 s (CPU 1.64 s) |
| The same, ten mutated copies of each sequence (26,370 sequences, from `scripts/bench/make_data.py`) | 27 | 579 | 2.53 s (CPU 6.11 s) | 2.54 s (CPU 6.17 s) |
| MERS-CoV spike gene (`tests/data/MERS_Spike_nt.fasta.bz2`, 499 sequences), results discarded with `-o /dev/null -O json` | 1,000 | 123 | 0.74 s (CPU 1.24 s) | 0.93 s (CPU 1.88 s) |

At $k = 27$ the option added about 0.05 s of CPU time and almost nothing to the wall time. Peak
memory did not change measurably: about 120 MB, 265 MB and 630 MB with or without it. With long
k-mers its share grows, because every distinct k-mer costs $k$ steps: at $k = 1{,}000$ it
increased the CPU time by about half and the wall time by about a quarter. Long k-mers also make
most windows distinct, so the counting itself then needs much more memory (see
[Memory](#memory)); the option adds only its table.

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
