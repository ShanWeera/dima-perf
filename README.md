# DiMA - Diversity Motif Analyser (Native Rust)

A native Rust CLI and library for analyzing k-mer diversity in FASTA sequences. This is a conversion of the original PyO3-based Python extension to pure Rust.

## Features

- Analyze k-mer diversity in protein and nucleotide sequences
- Calculate Shannon entropy with statistical correction
- Identify motif variants (Index, Major, Minor, Unique)
- Export results to JSON or binary format
- Extract highly conserved sequences (HCS)
- High-performance parallel processing using Rayon
- **Robust character validation** with whitelist-based filtering
- Columnar metadata storage for improved performance
- Binary output format for 50-70% faster I/O

## CLI Usage

Install Rust first, then:

```bash
cargo build --release
./target/release/dima --help
```

### Basic Examples

```bash
# Full analysis to stdout
./target/release/dima analyze -i data/sequences.fasta -k 9 -t 30 -n "Sample 1"

# Save full results to a file
./target/release/dima analyze -i data/sequences.fasta -o results.json

# Provide header format and alphabet
./target/release/dima analyze -i data/sequences.fasta --header-format "country|date|patient" --alphabet nucleotide

# Extract HCS only (with optional threshold) and print to stdout
./target/release/dima analyze -i data/sequences.fasta --hcs --hcs-threshold 5

# Extract HCS and write to a file
./target/release/dima analyze -i data/sequences.fasta --hcs -o hcs.json
```

### Character Validation

DiMA uses **whitelist-based character validation** to ensure only valid biological sequences are processed. This prevents invalid characters like `#`, `*`, `@`, numbers, etc. from appearing in k-mers.

#### Validation Modes

```bash
# Strict mode (DEFAULT, RECOMMENDED): Only accept valid alphabet characters
# Protein: A, C, D, E, F, G, H, I, K, L, M, N, P, Q, R, S, T, V, W, Y (20 amino acids)
# Nucleotide: A, C, G, T, U (5 nucleotides)
./target/release/dima analyze -i sequences.fasta --validation strict

# Permissive mode: Accept valid + known ambiguous characters (X, B, N, etc.)
# Only completely invalid characters (#, *, etc.) cause NA k-mers
./target/release/dima analyze -i sequences.fasta --validation permissive

# Report mode: Accept all characters but report statistics about invalid ones
./target/release/dima analyze -i sequences.fasta --validation report --report-invalid
```

#### Additional Validation Options

```bash
# Allow lowercase characters (auto-converted to uppercase)
./target/release/dima analyze -i sequences.fasta --allow-lowercase

# Report statistics about invalid characters found
./target/release/dima analyze -i sequences.fasta --report-invalid
```

### Character Classification

| Category | Protein | Nucleotide | Behavior |
|----------|---------|------------|----------|
| **Valid** | `ACDEFGHIKLMNPQRSTVWY` | `ACGTU` | Encoded in k-mer |
| **Ambiguous** | `XBJZOU` | `RYKMSWBDHVN` | Causes NA in strict mode |
| **Gap** | `-` | `-` | Causes NA (alignment gap) |
| **Invalid** | All other characters | All other characters | Always causes NA + warning |

### Performance Options

```bash
# Use columnar metadata storage for improved performance
./target/release/dima analyze -i sequences.fasta --columnar

# Enable metadata indexing for faster lookups
./target/release/dima analyze -i sequences.fasta --indexing

# Use binary output format for faster I/O
./target/release/dima analyze -i sequences.fasta -o results --binary

# Specify compression level (0=none, 1=lz4, 2=zstd)
./target/release/dima analyze -i sequences.fasta -o results --binary --compression 2

# Decompress binary format back to JSON
./target/release/dima deflate -i results.dima -o results.json
```

### All CLI Options

```
Options:
  -i, --input <FASTA>          Path to the FASTA file
  -k, --kmer <INT>             K-mer length (default: 9)
  -t, --threshold <INT>        Support threshold (default: 30)
  -n, --name <STR>             Query/sample name (default: "Unknown Protein")
  --header-format <STR>        Header fields separated by '|'
  --metadata-fields <STR>      Restrict metadata to these fields (subset of header-format)
  --header-fillna <STR>        Fill NA for empty header fields (default: "Unknown")
  --alphabet <protein|nucleotide>  Sequence alphabet (default: protein)
  -o, --output <FILE>          Output JSON file path
  --hcs                        Output only Highly Conserved Sequences
  --hcs-threshold <FLOAT>      Minimum incidence for HCS items (percent)
  --no-metadata                Disable per-variant metadata aggregation
  --threads <INT>              Number of Rayon worker threads
  --columnar                   Use columnar metadata storage
  --indexing                   Enable metadata indexing
  --binary                     Use binary format for output
  --compression <0|1|2>        Binary format compression level

Character Validation Options:
  --validation <strict|permissive|report>  Character validation mode (default: strict)
  --allow-lowercase            Allow lowercase characters (converted to uppercase)
  --report-invalid             Report statistics about invalid characters
```

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
dima = { path = "." }
```

### Basic Usage (Recommended - Validated)

```rust
use dima::{get_results_objs_validated, AnalysisConfig, ValidationMode};

fn main() {
    // Default strict validation (recommended)
    let (results, _stats) = get_results_objs_validated(
        "path/to/sequences.fasta".to_string(),
        9,  // k-mer length
        30, // support threshold
        "Sample Name".to_string(),
        None, // header format
        Some("protein".to_string()), // alphabet
        None, // header fill NA
        None, // metadata fields
        None, // use default config (strict validation)
    );
    println!("{}", results);
}
```

### With Custom Validation Config

```rust
use dima::{get_results_objs_validated, AnalysisConfig, ValidationMode};

fn main() {
    let config = AnalysisConfig::new()
        .with_validation_mode(ValidationMode::Permissive)
        .with_allow_lowercase(true)
        .with_report_invalid(true);
    
    let (results, stats) = get_results_objs_validated(
        "path/to/sequences.fasta".to_string(),
        9, 30,
        "Sample Name".to_string(),
        None, Some("protein".to_string()), None, None,
        Some(config),
    );
    
    // Print validation statistics if available
    if let Some(validation_stats) = stats {
        eprintln!("{}", validation_stats.summary());
    }
    
    println!("{}", results);
}
```

### Direct Character Validation

```rust
use dima::{CharacterValidator, AlphabetType, ValidationMode};

fn main() {
    let validator = CharacterValidator::with_options(
        AlphabetType::Protein,
        ValidationMode::Strict,
        false, // don't allow lowercase
    );
    
    // Check individual characters
    assert!(validator.is_valid(b'A'));  // Valid amino acid
    assert!(!validator.is_valid(b'#')); // Invalid character
    assert!(!validator.is_valid(b'X')); // Ambiguous (invalid in strict mode)
    
    // Check entire k-mer window
    assert!(!validator.window_has_invalid(b"ACDEF"));  // All valid
    assert!(validator.window_has_invalid(b"ACD#F"));   // Contains invalid
}
```

## Building

```bash
cargo build --release
```

## Testing

```bash
cargo test
```

## Why Whitelist-Based Validation?

The original implementation used a blacklist approach that only filtered known ambiguous characters. This allowed unexpected characters like `#`, `*`, `@`, numbers, etc. to slip through into k-mers.

The new whitelist approach:
- **Only allows valid biological characters** (20 amino acids or 5 nucleotides)
- **Explicitly handles ambiguous characters** with proper classification
- **Rejects all unknown characters** by default
- **Provides detailed reporting** of data quality issues
- **Is configurable** for different use cases

## Author

Shan Tharanga <stwm2@student.london.ac.uk>

## License

MIT License
