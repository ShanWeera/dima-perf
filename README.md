# DiMA - Diversity Motif Analyser (Native Rust)

A native Rust CLI and library for analyzing k-mer diversity in FASTA sequences. This is a conversion of the original PyO3-based Python extension to pure Rust.

## Features

- Analyze k-mer diversity in protein and nucleotide sequences
- Calculate Shannon entropy with statistical correction
- Identify motif variants (Index, Major, Minor, Unique)
- Export results to JSON format
- Extract highly conserved sequences (HCS)
- High-performance parallel processing using Rayon

## CLI Usage

Install Rust first, then:

```bash
cargo build --release
./target/release/dima --help | cat
```

Examples:

```bash
# Full analysis to stdout
./target/release/dima -i data/sequences.fasta -k 9 -t 30 -n "Sample 1"

# Save full results to a file
./target/release/dima -i data/sequences.fasta -o results.json

# Provide header format and alphabet
./target/release/dima -i data/sequences.fasta --header-format "country|date|patient" --alphabet nucleotide

# Extract HCS only (with optional threshold) and print to stdout
./target/release/dima -i data/sequences.fasta --hcs --hcs-threshold 5

# Extract HCS and write to a file
./target/release/dima -i data/sequences.fasta --hcs -o hcs.json
```

Options:

- `-i, --input <FASTA>`: Input FASTA file
- `-k, --kmer <INT>`: K-mer length (default: 9)
- `-t, --threshold <INT>`: Support threshold (default: 30)
- `-n, --name <STR>`: Query/sample name (default: "Unknown Protein")
- `--header-format <STR>`: Header fields separated by `|`, e.g., `country|date|patient`
- `--header-fillna <STR>`: Fill NA for empty header fields (default: `Unknown`)
- `--alphabet <protein|nucleotide>`: Sequence alphabet (default: protein)
- `-o, --output <FILE>`: Output JSON file path
- `--hcs`: Output only Highly Conserved Sequences
- `--hcs-threshold <FLOAT>`: Minimum incidence for HCS items (percent)

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
dima = { path = "." }
```

Then:

```rust
use dima::get_results_objs;

fn main() {
    let results = get_results_objs(
        "path/to/sequences.fasta".to_string(),
        9,  // k-mer length
        30, // support threshold
        "Sample Name".to_string(),
        None, // header format
        Some("protein".to_string()), // alphabet (protein or nucleotide)
        None  // header fill NA
    );
    println!("{}", results);
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

## Author

Shan Tharanga <stwm2@student.london.ac.uk>

## License

MIT License 