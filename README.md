# DiMA - Diversity Motif Analyser

A high-performance command-line tool for analyzing protein and nucleotide sequence diversity using k-mer based entropy analysis.

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
  - [Working with Metadata](#working-with-metadata)
  - [Extracting Conserved Sequences](#extracting-conserved-sequences)
  - [Output Formats](#output-formats)
  - [Performance Optimization](#performance-optimization)
  - [Character Validation](#character-validation)
- [Command Reference](#command-reference)
  - [analyze Command](#analyze-command)
  - [deflate Command](#deflate-command)
- [Output Format](#output-format)
  - [JSON Structure](#json-structure)
  - [Understanding the Results](#understanding-the-results)
- [Examples](#examples)
  - [Basic Example](#basic-example)
  - [With Metadata Aggregation](#with-metadata-aggregation)
  - [Highly Conserved Sequences](#highly-conserved-sequences)
  - [Binary Output for Large Datasets](#binary-output-for-large-datasets)
- [Performance](#performance)
- [Publications](#publications)
- [License](#license)

---

## Overview

### What is DiMA?

Protein sequence diversity is one of the major challenges in the design of diagnostic, prophylactic, and therapeutic interventions against viruses. **DiMA** (Diversity Motif Analyser) is a tool designed to facilitate the dissection of protein sequence diversity dynamics.

DiMA provides a quantitative measure of sequence diversity by using **Shannon's entropy**, applied via a user-defined k-mer sliding window. The entropy value is corrected for sample size bias by applying a statistical adjustment through linear regression extrapolation.

### Key Features

- **K-mer Sliding Window Analysis**: Analyze sequence diversity at each position using configurable k-mer lengths
- **Shannon's Entropy**: Quantify diversity with sample-size corrected entropy calculations
- **Diversity Motif Classification**: Automatically classify variants as Index, Major, Minor, or Unique
- **Metadata Aggregation**: Track the distribution of metadata (country, date, host, etc.) per variant
- **Highly Conserved Sequences (HCS)**: Extract conserved regions for vaccine design and epitope mapping
- **High Performance**: Written in Rust with parallel processing, capable of analyzing millions of sequences
- **Flexible Output**: JSON and compact binary formats with optional compression
- **Strict Validation**: Configurable character validation with detailed reporting

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

**Linux / macOS:**
```bash
# Download and extract
curl -LO https://github.com/BVU-BILSAB/DiMA/releases/latest/download/dima-<version>-<platform>.tar.gz
tar -xzf dima-*.tar.gz

# Move to PATH
sudo mv dima-*/dima /usr/local/bin/

# Verify installation
dima --help
```

**Windows (PowerShell):**
```powershell
# Download and extract
Invoke-WebRequest -Uri "https://github.com/BVU-BILSAB/DiMA/releases/latest/download/dima-<version>-x86_64-pc-windows-msvc.zip" -OutFile "dima.zip"
Expand-Archive -Path "dima.zip" -DestinationPath "."

# Add to PATH or run directly
.\dima.exe --help
```

### Build from Source

Requires [Rust](https://rustup.rs/) 1.70 or later.

```bash
# Clone the repository
git clone https://github.com/BVU-BILSAB/DiMA.git
cd DiMA

# Build release binary
cargo build --release

# Binary is at ./target/release/dima
./target/release/dima --help
```

---

## Quick Start

```bash
# Basic analysis
dima analyze -i sequences.fasta -o results.json

# With metadata extraction
dima analyze -i sequences.fasta -o results.json --header-format "accession|country|date"

# Extract highly conserved sequences
dima analyze -i sequences.fasta --hcs --hcs-threshold 95

# High-performance mode for large datasets
dima analyze -i large_dataset.fasta -o results --columnar --binary
```

---

## Usage Guide

### Basic Analysis

The simplest usage analyzes a FASTA file and outputs diversity metrics:

```bash
dima analyze -i aligned_sequences.fasta -o results.json
```

**Input requirements:**
- FASTA file with **aligned sequences** (all sequences must be the same length)
- Sequences can be protein (default) or nucleotide

**Key parameters:**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `-k, --kmer` | 9 | K-mer length (sliding window size) |
| `-t, --threshold` | 30 | Support threshold for entropy extrapolation |
| `-n, --name` | "Unknown Protein" | Sample/query name in output |
| `--alphabet` | protein | Sequence type: `protein` or `nucleotide` |

### Working with Metadata

FASTA headers often contain metadata separated by pipes (`|`). DiMA can parse this metadata and aggregate it per variant:

**Example FASTA header:**
```
>USA|2023-01-15|Human|Delta
MFVFLVLLPLVSSQCVNLTTRTQLPPAYTNS...
```

**Command:**
```bash
dima analyze -i sequences.fasta -o results.json \
  --header-format "country|date|host|variant"
```

**Filtering metadata fields:**

To aggregate only specific fields (reducing memory and output size):

```bash
dima analyze -i sequences.fasta -o results.json \
  --header-format "country|date|host|variant" \
  --metadata-fields "country|variant"
```

**Handling missing values:**

```bash
dima analyze -i sequences.fasta -o results.json \
  --header-format "country|date|host" \
  --header-fillna "Unknown"
```

### Extracting Conserved Sequences

Highly Conserved Sequences (HCS) are regions where the same k-mer appears most frequently across all sequences. These are valuable for vaccine design and identifying stable epitopes.

```bash
# Extract all conserved sequences
dima analyze -i sequences.fasta --hcs

# Only sequences present in ≥95% of samples
dima analyze -i sequences.fasta --hcs --hcs-threshold 95

# Save to file
dima analyze -i sequences.fasta --hcs --hcs-threshold 95 -o conserved.json
```

### Output Formats

**JSON (default):**
```bash
dima analyze -i sequences.fasta -o results.json
```

**Binary format** (50-70% faster I/O, 90%+ smaller files):
```bash
# With LZ4 compression (default, best balance)
dima analyze -i sequences.fasta -o results --binary

# With Zstd compression (maximum compression)
dima analyze -i sequences.fasta -o results --binary --compression 2

# No compression (fastest)
dima analyze -i sequences.fasta -o results --binary --compression 0
```

**Converting binary back to JSON:**
```bash
dima deflate -i results.dima -o results.json
```

### Performance Optimization

For large datasets (>100,000 sequences), use these optimizations:

```bash
# Columnar storage (14% faster with metadata)
dima analyze -i large.fasta -o results.json --columnar --header-format "..."

# Combined optimizations
dima analyze -i large.fasta -o results --columnar --binary --header-format "..."

# Limit thread count
dima analyze -i large.fasta -o results.json --threads 4
```

| Flag | Effect | Best For |
|------|--------|----------|
| `--columnar` | 14-17% faster metadata processing | Large datasets with metadata |
| `--binary` | 90%+ smaller output, faster I/O | Storage, archival, transfer |
| `--threads N` | Limit parallelism | Shared systems, containers |

### Character Validation

DiMA validates sequence characters to ensure data quality:

**Validation modes:**

| Mode | Description |
|------|-------------|
| `strict` (default) | Only canonical characters (20 amino acids or ACGTU) |
| `permissive` | Also accepts IUPAC ambiguity codes (X, B, N, etc.) |
| `report` | Accepts all, tracks statistics |

```bash
# Strict mode (recommended)
dima analyze -i sequences.fasta -o results.json --validation strict

# Allow ambiguous codes
dima analyze -i sequences.fasta -o results.json --validation permissive

# Handle lowercase input
dima analyze -i sequences.fasta -o results.json --allow-lowercase

# Report validation statistics
dima analyze -i sequences.fasta -o results.json --report-invalid
```

**Valid characters:**

| Alphabet | Valid | Ambiguous (permissive mode) |
|----------|-------|----------------------------|
| Protein | `ACDEFGHIKLMNPQRSTVWY` | `XBJZOU` |
| Nucleotide | `ACGTU` | `RYKMSWBDHVN` |

---

## Command Reference

### analyze Command

Analyze a FASTA file and generate diversity motif results.

```
dima analyze [OPTIONS] --input <FASTA>
```

**Required:**

| Option | Description |
|--------|-------------|
| `-i, --input <FASTA>` | Path to the aligned FASTA file |

**Analysis Parameters:**

| Option | Default | Description |
|--------|---------|-------------|
| `-k, --kmer <N>` | 9 | K-mer length for sliding window |
| `-t, --threshold <N>` | 30 | Support threshold for entropy calculation |
| `-n, --name <NAME>` | "Unknown Protein" | Sample/query name |
| `--alphabet <TYPE>` | protein | `protein` or `nucleotide` |

**Metadata Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--header-format <FMT>` | - | Pipe-separated field names (e.g., `"country\|date\|host"`) |
| `--metadata-fields <FMT>` | - | Subset of fields to aggregate |
| `--header-fillna <VAL>` | "Unknown" | Replacement for empty fields |

**Output Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `-o, --output <FILE>` | stdout | Output file path |
| `--hcs` | false | Output only Highly Conserved Sequences |
| `--hcs-threshold <N>` | - | Minimum incidence % for HCS (0-100) |
| `--binary` | false | Use binary format (.dima) |
| `--compression <N>` | 1 | Binary compression: 0=none, 1=LZ4, 2=Zstd |

**Performance Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--threads <N>` | all CPUs | Number of parallel threads |
| `--columnar` | false | Use columnar storage (faster with metadata) |
| `--indexing` | false | Enable metadata indexing |

**Validation Options:**

| Option | Default | Description |
|--------|---------|-------------|
| `--validation <MODE>` | strict | `strict`, `permissive`, or `report` |
| `--allow-lowercase` | false | Convert lowercase to uppercase |
| `--report-invalid` | false | Print validation statistics to stderr |

### deflate Command

Convert binary format (.dima) back to JSON.

```
dima deflate [OPTIONS] --input <BINARY>
```

| Option | Description |
|--------|-------------|
| `-i, --input <BINARY>` | Path to the .dima file |
| `-o, --output <FILE>` | Output JSON file (stdout if not specified) |
| `--no-pretty` | Output compact JSON without formatting |

---

## Output Format

### JSON Structure

```json
{
  "sequence_count": 1000,
  "support_threshold": 30,
  "low_support_count": 5,
  "query_name": "SARS-CoV-2 Spike",
  "kmer_length": 9,
  "average_entropy": 0.156,
  "highest_entropy_position": 484,
  "highest_entropy_value": 2.31,
  "results": [
    {
      "position": 1,
      "entropy": 0.0,
      "support": 1000,
      "low_support": null,
      "distinct_variants_count": 1,
      "distinct_variants_incidence": 100.0,
      "total_variants_incidence": 0.0,
      "diversity_motifs": [
        {
          "sequence": "MFVFLVLLP",
          "count": 998,
          "incidence": 99.8,
          "motif_short": "I",
          "motif_long": "Index",
          "metadata": {
            "country": {"USA": 450, "UK": 300, "Germany": 248},
            "date": {"2023-01": 500, "2023-02": 498}
          }
        },
        {
          "sequence": "MFVFLVLLQ",
          "count": 2,
          "incidence": 0.2,
          "motif_short": "U",
          "motif_long": "Unique",
          "metadata": {
            "country": {"USA": 2},
            "date": {"2023-01": 2}
          }
        }
      ]
    }
  ]
}
```

### Understanding the Results

**Top-level fields:**

| Field | Description |
|-------|-------------|
| `sequence_count` | Total number of sequences analyzed |
| `support_threshold` | Minimum support for reliable entropy |
| `low_support_count` | Positions with support below threshold |
| `kmer_length` | K-mer window size used |
| `average_entropy` | Mean entropy across all positions |
| `highest_entropy_position` | Position with maximum diversity |
| `highest_entropy_value` | Maximum entropy value |

**Per-position fields:**

| Field | Description |
|-------|-------------|
| `position` | 1-indexed position in the alignment |
| `entropy` | Shannon's entropy (0 = conserved, higher = diverse) |
| `support` | Number of valid k-mers at this position |
| `low_support` | Label if support is low: `NS`, `LS`, or `ELS` |
| `distinct_variants_count` | Number of unique k-mer sequences (excluding Index) |
| `diversity_motifs` | Array of variant details |

**Low support labels:**

| Label | Meaning |
|-------|---------|
| `NS` | No Support (support = 0) |
| `LS` | Low Support (support < threshold) |
| `ELS` | Exactly Low Support (support = threshold) |
| `null` | Normal support (support > threshold) |

---

## Examples

### Basic Example

```bash
# Analyze protein sequences with default settings
dima analyze -i spike_protein.fasta -o results.json -n "SARS-CoV-2 Spike"
```

### With Metadata Aggregation

```bash
# Header format: >USA|2023-01-15|Human|Delta
dima analyze -i sequences.fasta -o results.json \
  --header-format "country|date|host|variant" \
  --name "Global SARS-CoV-2"
```

### Highly Conserved Sequences

```bash
# Extract sequences conserved in ≥98% of samples
dima analyze -i sequences.fasta --hcs --hcs-threshold 98 -o conserved.json
```

Output:
```json
["MFVFLVLLPLVSSQCVNLTTRTQLPPAYTN", "FQFCNDPFLGVYYHKNNKSW"]
```

### Binary Output for Large Datasets

```bash
# Analyze with maximum performance
dima analyze -i large_dataset.fasta \
  -o results \
  --columnar \
  --binary \
  --compression 2 \
  --header-format "country|date" \
  --name "Large Analysis"

# Later, convert to JSON if needed
dima deflate -i results.dima -o results.json
```

---

## Performance

DiMA is optimized for high-throughput analysis. Benchmark on 2.3M sequences (Apple M3 Pro, 11 cores):

| Configuration | Runtime | Output Size |
|---------------|---------|-------------|
| Baseline (no metadata) | 128s | 29 MB |
| With metadata | 207s | 132 MB |
| With metadata + `--columnar` | 178s | 132 MB |
| With metadata + `--columnar --binary` | 179s | 15 MB |

**Key insights:**
- `--columnar` provides **14% speedup** when processing metadata
- `--binary --compression 1` reduces output size by **90%**
- Without metadata, baseline is fastest (no optimization needed)

See [BENCHMARK.md](BENCHMARK.md) for detailed performance analysis.

---

## Publications

- DiMA: A tool for analysis of viral protein sequence diversity dynamics  
  https://arxiv.org/abs/2205.13915

---

## License

MIT License - see [LICENSE](LICENSE) for details.
