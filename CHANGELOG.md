# Changelog

All notable changes to DiMA will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Core diversity analysis engine with Shannon's entropy and sample-size bias correction
- Diversity motif classification (Index, Major, Minor, Unique)
- HCS (Highly Conserved Sequences) extraction with threshold-based filtering and stitching
- Multiple output formats: JSON, TSV (17-column), JSONL, binary `.dima`
- Binary format with LZ4/Zstd compression and string interning
- Transparent decompression of gz/bz2/xz/zst input files via needletail
- Metadata parsing and per-variant aggregation with automatic columnar storage
- Memory-mapped I/O with configurable heuristic (`DIMA_FORCE_MMAP`)
- Disk-backed matrix mode for datasets exceeding available RAM
- Parallel computation via Rayon thread pool
- Character validation modes (strict/permissive/report) with configurable lowercase handling
- Shell completion generation for bash/zsh/fish/powershell/elvish
- Semantic exit codes (0=success, 1=runtime, 2=usage, 3=io, 130=cancelled)
- Cooperative cancellation via Ctrl+C with double-press force quit
- `view` command for binary format conversion and re-compression
- Desktop application (Tauri) with interactive visualization
- Atomic file writes (write-to-temp then rename) preventing corrupt output
- SIMD-accelerated string operations for sequence processing
- Comprehensive CLI help text with short (-h) and long (--help) variants
