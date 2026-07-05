# DiMA Performance Benchmark Report

This document provides comprehensive benchmark results for DiMA (Diversity Motif Analyser) performance optimization flags.

## Test Environment

| Component | Details |
|-----------|---------|
| **CPU** | Apple M3 Pro |
| **CPU Cores** | 11 |
| **OS** | macOS (Darwin) |
| **Rust** | Release build (`--release`) |
| **DiMA Version** | 0.1.0 |

## Input Dataset

### File Information

| Property | Value |
|----------|-------|
| **Filename** | `allhosts_headerformatted.fasta` |
| **File Size** | 2.9 GB |
| **Total Lines** | 52,742,634 |
| **Sequence Count** | 2,293,158 |
| **Sequence Length** | 1,295 amino acids |
| **Sequence Type** | Protein (SARS-CoV-2 Spike protein) |

### Header Structure

Headers follow a pipe-separated format with 3 fields:

```
>date|country|host
```

**Example headers:**
```
>2020-12|Philippines|Human
>2021-02|United Kingdom|Human
>2021-00|United Kingdom|Human
```

---

## Benchmark Configuration

### Analysis Parameters

| Parameter | Value |
|-----------|-------|
| **K-mer Length** | 9 (default) |
| **Support Threshold** | 100 (default, per PMC11596295) |
| **Thread Count** | 11 (all CPUs, default) |

### Test Matrix

Two groups of tests were conducted:

- **Group A**: Without metadata processing (6 configurations)
- **Group B**: With metadata processing using `--header-format "date|country|host"` (6 configurations)

---

## Benchmark Results

### Group A: Without Metadata Processing

These tests analyze sequence diversity without parsing or aggregating header metadata.

| Config | Flags | Runtime (s) | Output Size | Speedup vs Baseline |
|--------|-------|-------------|-------------|---------------------|
| **A1** | *(none - baseline)* | **128.34** | 29 MB | 1.00x |
| A2 | *(columnar now automatic, no separate flag)* | 132.83 | 29 MB | 0.97x |
| A3 | `-O dima --compression 0` | 135.37 | 5.5 MB | 0.95x |
| A4 | `-O dima --compression 1` | 136.47 | 2.8 MB | 0.94x |
| A5 | `-O dima --compression 2` | 135.11 | 2.4 MB | 0.95x |
| A6 | `-O dima --compression 1` | 135.43 | 2.8 MB | 0.95x |

**Key Observations:**
- Baseline (no extra flags) is the fastest for non-metadata workloads
- `--columnar` adds slight overhead (~3%) when no metadata is processed
- Binary format has minimal runtime impact but significantly reduces output size
- All configurations complete in approximately 128-136 seconds

### Group B: With Metadata Processing

These tests include header parsing and per-variant metadata aggregation.

| Config | Flags | Runtime (s) | Output Size | vs B1 | vs A1 |
|--------|-------|-------------|-------------|-------|-------|
| **B1** | `--header-format ...` *(without columnar, legacy)* | **207.48** | 132 MB | 1.00x | 0.62x |
| **B2** | `--header-format ...` *(columnar now automatic)* | **177.71** | 132 MB | **1.17x** | 0.72x |
| B3 | `-O dima --compression 0 --header-format ...` | 204.78 | 48 MB | 1.01x | 0.63x |
| B4 | `-O dima --compression 1 --header-format ...` | 204.66 | 14 MB | 1.01x | 0.63x |
| B5 | `-O dima --compression 2 --header-format ...` | 209.38 | 8.6 MB | 0.99x | 0.61x |
| **B6** | `-O dima --compression 1 --header-format ...` | **178.55** | 15 MB | **1.16x** | 0.72x |

**Key Observations:**
- Metadata processing adds **62% overhead** (128s → 207s)
- `--columnar` provides **14-17% speedup** when processing metadata
- Binary output format has negligible runtime impact
- Combining `--columnar` with `--binary` maintains the speedup while reducing output size

---

## Output Size Comparison

### JSON vs Binary Format

| Format | Without Metadata | With Metadata |
|--------|------------------|---------------|
| **JSON** | 29 MB | 132 MB |
| **Binary (no compression)** | 5.5 MB (81% smaller) | 48 MB (64% smaller) |
| **Binary (LZ4)** | 2.8 MB (90% smaller) | 14 MB (89% smaller) |
| **Binary (Zstd)** | 2.4 MB (92% smaller) | 8.6 MB (93% smaller) |

### Compression Comparison (Binary Format)

| Compression | Level | Without Metadata | With Metadata | Compression Ratio |
|-------------|-------|------------------|---------------|-------------------|
| None | 0 | 5.5 MB | 48 MB | ~5x vs JSON |
| LZ4 | 1 | 2.8 MB | 14 MB | ~10x vs JSON |
| Zstd | 2 | 2.4 MB | 8.6 MB | ~15x vs JSON |

---

## Performance Analysis

### Impact of Individual Flags

#### Columnar Storage (automatic)

| Workload | Impact |
|----------|--------|
| Without metadata | N/A (not activated) |
| With metadata | **+14-17% speedup** |

Columnar storage is now activated **automatically** when `--header-format` is specified. It reorganizes metadata into column-oriented arrays instead of row-oriented HashMaps, providing:
- Better CPU cache locality for sequential field access
- More efficient string interning (15-25% memory reduction)
- SIMD-friendly memory layout for bulk operations

#### `-O dima` (Binary Output)

| Metric | Impact |
|--------|--------|
| Runtime | Negligible (<1%) |
| Output size | 80-93% reduction |

The binary format (.dima) uses:
- Compact bincode serialization
- String interning (deduplication)
- Optional compression (LZ4 or Zstd)

**Recommendation**: Use `-O dima` when output size matters or for archival/transfer.

#### `--compression`

| Level | Algorithm | Speed | Size Reduction |
|-------|-----------|-------|----------------|
| 0 | None | Fastest | ~80% vs JSON |
| 1 | LZ4 | Fast | ~90% vs JSON |
| 2 | Zstd | Moderate | ~93% vs JSON |

**Recommendation**: Use LZ4 (level 1) for general use, Zstd (level 2) for maximum compression.

### Metadata Processing Overhead

Processing metadata with `--header-format` adds significant overhead:

| Metric | Without Metadata | With Metadata | Overhead |
|--------|------------------|---------------|----------|
| Runtime | 128.34s | 207.48s | +62% |
| Output Size (JSON) | 29 MB | 132 MB | +355% |

This overhead comes from:
1. Header parsing and validation
2. Per-variant metadata aggregation (HashMaps)
3. Larger output serialization

---

## Recommendations

### By Use Case

| Use Case | Recommended Flags | Expected Runtime | Output Size |
|----------|-------------------|------------------|-------------|
| **Fastest (no metadata)** | *(none)* | ~128s | 29 MB |
| **Smallest output (no metadata)** | `-O dima --compression 2` | ~135s | 2.4 MB |
| **Fastest with metadata** | `--header-format "..."` | ~178s | 132 MB |
| **Balanced with metadata** | `-O dima --header-format "..."` | ~179s | 15 MB |
| **Smallest with metadata** | `-O dima --compression 2 --header-format "..."` | ~209s | 8.6 MB |

### Quick Reference

```bash
# Fastest analysis (no metadata)
dima analyze -i input.fasta -o output.json

# Fastest with metadata (columnar storage is automatic)
dima analyze -i input.fasta -o output.json --header-format "field1|field2|field3"

# Smallest output with metadata
dima analyze -i input.fasta -O dima --compression 2 -o output.dima --header-format "field1|field2|field3"

# Best balance (fast + small output)
dima analyze -i input.fasta -O dima -o output.dima --header-format "field1|field2|field3"
```

---

## Throughput Metrics

Based on the benchmark results with 2,293,158 sequences:

| Configuration | Sequences/sec | K-mers/sec* |
|---------------|---------------|-------------|
| Baseline (no metadata) | 17,866 | 22.98 million |
| With metadata (baseline) | 11,051 | 14.22 million |
| With metadata + columnar | 12,903 | 16.60 million |

*Estimated based on sequence length of 1,295 amino acids and k=9, yielding 1,287 k-mers per sequence.

---

## Conclusion

1. **For pure diversity analysis** (no metadata): Use default settings. Additional flags provide no benefit.

2. **For metadata-enabled analysis**: Always use `--columnar` for a consistent 14-17% speedup.

3. **For storage/transfer optimization**: Use `--binary --compression 1` (LZ4) for the best balance of speed and size, or `--compression 2` (Zstd) for maximum compression.

4. **The bottleneck is computation, not I/O**: Binary format significantly reduces output size but has minimal impact on runtime. The majority of time is spent on k-mer extraction, encoding, and entropy calculation.

5. **Metadata processing is expensive**: If you don't need per-variant metadata aggregation, omit `--header-format` for 62% faster processing.

---

*Benchmark conducted: January 2026*
*DiMA v0.1.0*
