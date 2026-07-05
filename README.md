# DiMA - Diversity Motif Analyser

A high-performance command-line tool for analyzing protein and nucleotide sequence diversity using k-mer based entropy analysis. Implements the methodology from [Tharanga et al. (2025, PMC11596295)](https://doi.org/10.1093/bioadv/vbae607).

---

## Table of Contents

- [Overview](#overview)
  - [What is DiMA?](#what-is-dima)
  - [Key Features](#key-features)
  - [Diversity Motifs](#diversity-motifs)
- [Installation](#installation)
  - [Pre-built Binaries](#pre-built-binaries)
  - [Build from Source](#build-from-source)
- [Quick Start](#quick-start)
- [Usage Guide](#usage-guide)
  - [Basic Analysis](#basic-analysis)
  - [Input Sources](#input-sources)
  - [Output Formats](#output-formats)
  - [Working with Metadata](#working-with-metadata)
  - [Extracting Conserved Sequences](#extracting-conserved-sequences)
  - [Performance Optimization](#performance-optimization)
  - [Character Validation](#character-validation)
- [Command Reference](#command-reference)
  - [analyze Command](#analyze-command)
  - [view Command](#view-command)
- [Output Format](#output-format)
  - [JSON Structure](#json-structure)
  - [TSV Structure](#tsv-structure)
  - [Understanding the Results](#understanding-the-results)
- [Examples](#examples)
- [Performance](#performance)
- [Environment Variables](#environment-variables)
- [Publications](#publications)
- [License](#license)

---

## Overview

### What is DiMA?

Protein sequence diversity is one of the major challenges in the design of diagnostic, prophylactic, and therapeutic interventions against viruses. **DiMA** (Diversity Motif Analyser) is a tool designed to facilitate the dissection of protein sequence diversity dynamics.

DiMA provides a quantitative measure of sequence diversity by using **Shannon's entropy**, applied via a user-defined k-mer sliding window. The entropy value is corrected for sample size bias by applying a statistical adjustment through linear regression extrapolation.

### Key Features

- **K-mer Sliding Window Analysis**: Analyze sequence diversity at each position using configurable k-mer lengths
- **Shannon's Entropy**: Quantify diversity with sample-size corrected entropy calculations (per PMC11596295)
- **Diversity Motif Classification**: Automatically classify variants as Index, Major, Minor, or Unique
- **Metadata Aggregation**: Track the distribution of metadata (country, date, host, etc.) per variant
- **Highly Conserved Sequences (HCS)**: Extract conserved regions for vaccine design and epitope mapping
- **Multiple Output Formats**: JSON, TSV (17-column vDiveR-aligned), JSONL, and binary `.dima` (via `-O`)
- **Compressed Input**: Transparent `.gz`, `.bz2`, `.xz`, `.zst` decompression
- **Stdin/Pipe Support**: Standard Unix piping workflows (`cat seqs.fasta | dima analyze`)
- **Adaptive Memory**: Auto-detects available RAM and can use disk-backed mode for huge datasets
- **High Performance**: Written in Rust with parallel processing (rayon), memory-mapped I/O, SIMD string ops
- **Strict Validation**: Configurable character validation with detailed reporting
- **Verbose Performance Reporting**: Phase timing and peak memory at `-v`

### Diversity Motifs

At each k-mer position, distinct sequences are classified into motifs based on their incidence:

| Motif | Short | Description |
|-------|-------|-------------|
| **Index** | I | The predominant sequence (highest count, appears more than once) |
| **Major** | Ma | The second most common sequence (after Index) |
| **Minor** | Mi | Sequences between Major and Unique in frequency |
| **Unique** | U | Sequences that appear only once |

---

## Installation

### Pre-built Binaries

Download the latest release for your platform from the [Releases](https://github.com/BVU-BILSAB/DiMA/releases) page.

### Build from Source

Requires [Rust](https://rustup.rs/) 1.81 or later.

```bash
git clone https://github.com/BVU-BILSAB/DiMA.git
cd DiMA

# Build optimized release binary
cargo build --release

# Binary is at ./target/release/dima
./target/release/dima --help
```

---

## Quick Start

```bash
# Basic analysis (JSON output)
dima analyze -i aligned.fasta -o results.json

# TSV output for R/Python workflows
dima analyze -i aligned.fasta -O tsv -o results.tsv

# Binary format for large datasets (compact, fast I/O)
dima analyze -i aligned.fasta -O dima -o results.dima

# Convert binary to other formats
dima view -i results.dima -O tsv -o results.tsv

# Pipe from compressed input
cat sequences.fasta.gz | dima analyze -k 9 -o results.json

# With metadata extraction
dima analyze -i aligned.fasta -o results.json --header-format "country|date|host"
```

---

## Usage Guide

### Basic Analysis

```bash
dima analyze -i aligned_sequences.fasta -o results.json
```

**Input requirements:**
- FASTA file with **aligned sequences** (all sequences must be the same length)
- Sequences can be protein (default) or nucleotide
- Compressed files (`.gz`, `.bz2`, `.xz`, `.zst`) are transparently decompressed

**Key parameters:**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `-k, --kmer` | 9 (protein) / 27 (nucleotide) | K-mer length (sliding window size) |
| `-t, --threshold` | 100 | Support threshold for entropy extrapolation |
| `-n, --name` | "Unknown Protein" | Sample/query name in output |
| `--alphabet` | protein | Sequence type: `protein` or `nucleotide` |

### Input Sources

DiMA supports multiple input methods:

```bash
# File input (most common)
dima analyze -i sequences.fasta -o results.json

# Compressed file input (transparent decompression)
dima analyze -i sequences.fasta.gz -o results.json

# Explicit stdin
dima analyze -i - -k 9 -o results.json < sequences.fasta

# Auto-detected piped stdin (--input omitted)
cat sequences.fasta.gz | dima analyze -k 9 -o results.json

# Pipe between tools
seqkit grep -p "spike" all_proteins.fasta | dima analyze -o spike_results.json
```

### Output Formats

DiMA supports four output formats, selected via `-O/--output-type`:

```bash
# JSON (default) — human-readable, pretty-printed to file, compact to pipe
dima analyze -i seqs.fasta -o results.json

# TSV — 17-column tab-separated for R/Python/Excel workflows
dima analyze -i seqs.fasta -O tsv -o results.tsv

# JSONL — one JSON object per position, for streaming/incremental processing
dima analyze -i seqs.fasta -O jsonl -o results.jsonl

# Binary .dima — compact format with LZ4/Zstd compression
dima analyze -i seqs.fasta -O dima -o results.dima
```

**Format auto-detection**: When `-O` is not specified, format is inferred from the output file extension:
- `.dima` → binary
- `.tsv` / `.tab` → TSV
- `.jsonl` / `.ndjson` → JSONL
- Everything else → JSON

**Compression** (binary format only):

```bash
dima analyze -i seqs.fasta -O dima -o results.dima --compression 0  # None
dima analyze -i seqs.fasta -O dima -o results.dima --compression 1  # LZ4 (default)
dima analyze -i seqs.fasta -O dima -o results.dima --compression 2  # Zstd (smallest)
```

### Working with Metadata

FASTA headers often contain metadata separated by pipes (`|`). DiMA can parse and aggregate metadata per variant:

```bash
# Header format: >USA|2023-01-15|Human|Delta
dima analyze -i sequences.fasta -o results.json \
  --header-format "country|date|host|variant"

# Aggregate only specific fields
dima analyze -i sequences.fasta -o results.json \
  --header-format "country|date|host|variant" \
  --metadata-fields "country|variant"
```

### Extracting Conserved Sequences

Highly Conserved Sequences (HCS) are regions where the same k-mer appears most frequently:

```bash
# Extract sequences conserved in ≥95% of samples
dima analyze -i sequences.fasta -o results.json \
  --hcs-output conserved.json --hcs-threshold 95
```

### Performance Optimization

```bash
# Limit thread count for shared systems
dima analyze -i large.fasta -o results.json --threads 4

# Verbose mode: shows phase timing and peak memory
dima analyze -i large.fasta -o results.json -v

# Quiet mode: suppress all non-error output
dima analyze -i large.fasta -o results.json -q

# Force disk-backed mode for very large datasets
dima analyze -i huge.fasta -o results.json --low-memory

# Override temp directory for disk-backed mode
dima analyze -i huge.fasta -o results.json --low-memory --temp-dir /scratch/tmp
```

### Character Validation

```bash
# Strict mode (default) — only canonical characters
dima analyze -i sequences.fasta -o results.json --validation strict

# Allow IUPAC ambiguity codes
dima analyze -i sequences.fasta -o results.json --validation permissive

# Handle mixed-case input
dima analyze -i sequences.fasta -o results.json --allow-lowercase

# Report character validation statistics
dima analyze -i sequences.fasta -o results.json --report-invalid
```

---

## Command Reference

### analyze Command

Analyze a FASTA file and generate diversity motif results.

```
dima analyze [OPTIONS] [-i <FASTA>]
```

**Input:**

| Option | Description |
|--------|-------------|
| `-i, --input <FASTA>` | Path to aligned FASTA file (or `-` for stdin). Omit when piping. |

**Analysis Parameters:**

| Option | Default | Description |
|--------|---------|-------------|
| `-k, --kmer <N>` | 9 / 27 | K-mer length (protein / nucleotide) |
| `-t, --threshold <N>` | 100 | Support threshold for entropy extrapolation |
| `-n, --name <NAME>` | auto | Sample/query name |
| `--alphabet <TYPE>` | protein | `protein` or `nucleotide` |

**Output Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-o, --output <FILE>` | stdout | Output file path |
| `-O, --output-type <FMT>` | auto | `json`, `tsv`, `jsonl`, or `dima` |
| `--no-header` | false | Omit TSV header row |
| `--compression <N>` | 1 | Binary compression: 0=none, 1=LZ4, 2=Zstd |
| `--hcs-output <FILE>` | - | HCS output file path |
| `--hcs-threshold <N>` | - | Min incidence % for HCS (0-100) |

**Metadata Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--header-format <FMT>` | - | Pipe-separated field names |
| `--metadata-fields <FMT>` | - | Subset of fields to aggregate |
| `--header-fillna <VAL>` | "Unknown" | Replacement for empty fields |

**Memory & Performance:**

| Option | Default | Description |
|--------|---------|-------------|
| `--threads <N>` | all CPUs | Number of parallel threads |
| `--low-memory` | false | Force disk-backed matrix storage |
| `--force-ram` | false | Force RAM mode (may OOM) |
| `--temp-dir <DIR>` | auto | Temp directory for disk-backed mode |

**Validation:**

| Option | Default | Description |
|--------|---------|-------------|
| `--validation <MODE>` | strict | `strict`, `permissive`, or `report` |
| `--allow-lowercase` | false | Convert lowercase to uppercase |
| `--report-invalid` | false | Print validation statistics |

**Verbosity:**

| Option | Effect |
|--------|--------|
| (default) | Warnings + progress bars |
| `-v` | + performance report + info messages |
| `-vv` | + debug messages |
| `-q` | Suppress all non-error output |

### view Command

View/convert binary `.dima` files to other formats. Follows the samtools/bcftools `view` convention.

```
dima view [OPTIONS] --input <DIMA_FILE>
```

| Option | Default | Description |
|--------|---------|-------------|
| `-i, --input <FILE>` | required | Path to `.dima` binary file |
| `-o, --output <FILE>` | stdout | Output file path |
| `-O, --output-type <FMT>` | json | `json`, `tsv`, `jsonl`, or `dima` |
| `--no-header` | false | Omit TSV header row |
| `--compression <N>` | 1 | Compression for `-O dima` re-encoding |

**Examples:**

```bash
# Convert to JSON
dima view -i results.dima -o results.json

# Convert to TSV
dima view -i results.dima -O tsv -o results.tsv

# Re-encode with different compression
dima view -i results.dima -O dima -o results_zstd.dima --compression 2
```

---

## Output Format

### JSON Structure

```json
{
  "sequence_count": 1000,
  "support_threshold": 100,
  "low_support_count": 5,
  "query_name": "SARS-CoV-2 Spike",
  "kmer_length": 9,
  "average_entropy": 0.156,
  "highest_entropy": { "position": 484, "entropy": 2.31 },
  "results": [
    {
      "position": 1,
      "entropy": 0.0,
      "support": 1000,
      "low_support": null,
      "distinct_variants_count": 1,
      "distinct_variants_incidence": 50.0,
      "total_variants_incidence": 0.2,
      "diversity_motifs": [
        {
          "sequence": "MFVFLVLLP",
          "count": 998,
          "incidence": 99.8,
          "motif_short": "I",
          "motif_long": "Index",
          "metadata": { "country": {"USA": 450, "UK": 300} }
        }
      ]
    }
  ]
}
```

### TSV Structure

17-column format aligned with vDiveR conventions. Missing values represented as `.` (VCF standard).

```
position  entropy  support  low_support  distinct_variants_count  distinct_variants_incidence  total_variants_incidence  index_sequence  index_count  index_incidence  major_sequence  major_count  major_incidence  minor_count  minor_incidence  unique_count  unique_incidence
1         0.000000 1000     .            1                        50.00                        0.20                      MFVFLVLLP       998          99.80            MFVFLVLLQ       2            0.20             .            .                .             .
```

Tied motifs are comma-separated within cells (VCF v4.5 convention):
```
index_sequence=ACDE,FGHI  index_count=100,100  index_incidence=33.33,33.33
```

### Understanding the Results

**Low support labels:**

| Label | Meaning |
|-------|---------|
| `NS` | No Support (support = 0) |
| `LS` | Low Support (support < threshold) |
| `ELS` | Exceptional Low Support (support = threshold) |
| `null` / `.` | Normal (support > threshold) |

---

## Examples

```bash
# Analyze with TSV output for R
dima analyze -i spike.fasta -O tsv -o results.tsv -n "SARS-CoV-2 Spike"

# Nucleotide analysis
dima analyze -i genome.fasta --alphabet nucleotide -o results.json

# Full pipeline: compressed input → analysis → binary output
dima analyze -i sequences.fasta.gz -O dima -o results.dima \
  --header-format "country|date" -n "Global Analysis"

# Extract and view conserved regions
dima analyze -i sequences.fasta --hcs-output hcs.json --hcs-threshold 98
```

---

## Performance

Benchmark on 2.29M sequences (2.9 GB, Apple M3 Pro, 11 cores):

| Metric | Value |
|--------|-------|
| Total runtime | ~130s |
| Peak memory | ~18 GB |
| Throughput | ~17,600 sequences/s |
| Output (JSON) | ~130 MB |
| Output (binary, LZ4) | ~15 MB |

Use `-v` to see detailed phase timing:

```
  Phase timing:
    I/O + validation:     45.23s  ( 34.8%)
    Entropy computation:  52.10s  ( 40.1%)
    Position building:    30.55s  ( 23.5%)
    Output serialization:  2.12s  (  1.6%)
  Resources:
    Peak memory:  18.23 GB
    Threads used: 11
    Input size:   2.90 GB (2,291,233 sequences, 1142 positions)
```

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `DIMA_FORCE_MMAP=1` | Force memory-mapped I/O regardless of file size |
| `DIMA_FORCE_MMAP=0` | Force buffered I/O regardless of file size |
| `RAYON_NUM_THREADS=N` | Set thread pool size (alternative to `--threads`) |
| `NO_COLOR=1` | Disable colored output ([no-color.org](https://no-color.org/)) |
| `TMPDIR` | Override temp directory for disk-backed mode |

---

## Desktop Application

DiMA Desktop is a cross-platform GUI built with [Tauri 2](https://v2.tauri.app/) and React.

See the `src-tauri/` and `ui/` directories for development details.

---

## Publications

- Tharanga, S. et al. (2025). DiMA: A tool for analysis of viral protein sequence diversity dynamics.  
  *Bioinformatics Advances*, 5(1), vbae607. [PMC11596295](https://doi.org/10.1093/bioadv/vbae607)

---

## License

MIT License - see [LICENSE](LICENSE) for details.
