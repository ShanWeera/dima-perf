//! CLI help text constants for the DiMA tool.
//!
//! This module centralizes all help text to keep main.rs clean and readable.
//! Each flag has both a short help (for -h) and a long help (for --help).

// =============================================================================
// ANALYZE COMMAND - Core Parameters
// =============================================================================

pub mod analyze {
    // -------------------------------------------------------------------------
    // Input File (-i, --input)
    // -------------------------------------------------------------------------
    pub const INPUT_HELP: &str = "Path to the input FASTA file containing aligned sequences";

    pub const INPUT_LONG_HELP: &str = r#"Path to the input FASTA file containing aligned sequences.

The FASTA file should contain multiple aligned sequences of the same length.
Each sequence header line starts with '>' followed by optional metadata.

Requirements:
  - All sequences MUST be aligned (same length)
  - Sequences shorter than the k-mer length will produce no k-mers
  - Headers can contain pipe-separated metadata (see --header-format)

Supported formats:
  - Standard FASTA (.fasta, .fa, .faa, .fna)
  - Multi-line sequences are automatically concatenated

Example file structure:
  >USA|2023-01-15|Patient001
  ACDEFGHIKLMNPQRSTVWY...
  >Canada|2023-02-20|Patient002
  ACDEFGHIKLMNPQRSTVWY..."#;

    // -------------------------------------------------------------------------
    // K-mer Length (-k, --kmer)
    // -------------------------------------------------------------------------
    pub const KMER_HELP: &str = "K-mer length for sliding window analysis";

    pub const KMER_LONG_HELP: &str = r#"K-mer length for sliding window analysis.

The k-mer length determines the size of the sliding window used to extract
overlapping subsequences from each aligned sequence. For a sequence of length
L, this generates (L - k + 1) k-mers at positions 0, 1, 2, ..., L-k.

How it works:
  - A sliding window of size k moves across each sequence
  - At each position, the k-mer is extracted and encoded
  - K-mers are compared across all sequences at the same position
  - Entropy is calculated based on k-mer diversity at each position

Recommended values:
  - k=9 (default): Standard for T-cell epitope analysis (nonamers)
  - k=8-11: Common range for immune epitope studies
  - k=3-6: Short motif discovery
  - k=15-20: High-specificity sequence matching

Technical limits (due to u64 encoding):
  - Protein sequences: maximum k ~ 13-14
  - Nucleotide sequences: maximum k ~ 27-28

Example: For k=9 on sequence "ACDEFGHIKLMNPQRS" (length 16):
  Position 0: ACDEFGHIK
  Position 1: CDEFGHIKL
  Position 2: DEFGHIKLM
  ...
  Position 7: HIKLMNPQR (last k-mer)"#;

    // -------------------------------------------------------------------------
    // Support Threshold (-t, --threshold)
    // -------------------------------------------------------------------------
    pub const THRESHOLD_HELP: &str = "Minimum support for reliable entropy calculation";

    pub const THRESHOLD_LONG_HELP: &str = r#"Minimum support threshold for reliable entropy calculation.

"Support" is the number of valid k-mers at a given position across all
sequences. This threshold affects both the entropy calculation method
and the low-support classification labels in the output.

How support affects entropy calculation:
  - support = 0:         Entropy = 0.0 (no data)
  - support = 1:         Entropy = 0.0 (single k-mer, no diversity)
  - support < threshold: Standard Shannon entropy
  - support >= threshold: Extrapolation method using linear regression
                         (more statistically robust for large samples)

Low-support classification labels in output:
  - "NS" (No Support):      support = 0 (no valid k-mers at position)
  - "LS" (Low Support):     support < threshold
  - "ELS" (Exactly Low):    support = threshold
  - No label:               support > threshold (normal)

Choosing a threshold:
  - Default (30): Good balance for most datasets
  - Lower (10-20): Use for smaller datasets (<100 sequences)
  - Higher (50-100): Use for large datasets (>10,000 sequences)

The extrapolation method samples entropy at 5% intervals and uses linear
regression to estimate the true entropy, reducing bias from sample size."#;

    // -------------------------------------------------------------------------
    // Query Name (-n, --name)
    // -------------------------------------------------------------------------
    pub const NAME_HELP: &str = "Name identifier for this analysis";

    pub const NAME_LONG_HELP: &str = r#"Name identifier for this analysis.

This name is stored in the output JSON/binary as "query_name" and helps
identify the dataset when managing multiple analysis results.

Examples:
  --name "SARS-CoV-2 Spike Protein"
  --name "Influenza H1N1 HA"
  --name "HIV-1 Env Glycoprotein"

The name appears in the output as:
  {
    "query_name": "SARS-CoV-2 Spike Protein",
    "sequence_count": 1000,
    ...
  }"#;

    // -------------------------------------------------------------------------
    // Header Format (--header-format)
    // -------------------------------------------------------------------------
    pub const HEADER_FORMAT_HELP: &str = "Header format: pipe-separated field names (e.g., \"country|date|patient\")";

    pub const HEADER_FORMAT_LONG_HELP: &str = r#"Define the pipe-separated format of FASTA header metadata.

FASTA headers often contain metadata separated by pipe '|' characters.
This option tells DiMA how to parse and name each field for aggregation.

Format specification:
  - Fields are separated by '|' in the format string
  - The format MUST match the number of '|' separators in headers
  - Whitespace around field names is automatically trimmed
  - Field names become keys in the metadata aggregation output

Example FASTA header:
  >USA|2023-01-15|Patient001|Delta

Matching format:
  --header-format "country|date|patient_id|variant"

This produces metadata aggregation per k-mer variant:
  {
    "sequence": "ACDEFGHIK",
    "metadata": {
      "country": {"USA": 45, "Canada": 30, "Mexico": 25},
      "date": {"2023-01-15": 50, "2023-02-20": 50},
      "variant": {"Delta": 60, "Omicron": 40}
    }
  }

Important:
  - If format field count != header field count, analysis will fail
  - Empty fields are replaced with --header-fillna value (default: "Unknown")
  - Use --metadata-fields to aggregate only specific fields
  - Use --no-metadata to skip all header parsing (faster)"#;

    // -------------------------------------------------------------------------
    // Metadata Fields (--metadata-fields)
    // -------------------------------------------------------------------------
    pub const METADATA_FIELDS_HELP: &str = "Restrict aggregation to these fields only (subset of header-format)";

    pub const METADATA_FIELDS_LONG_HELP: &str = r#"Restrict metadata aggregation to specific fields only.

When processing large datasets with many metadata fields, you may only
need aggregation for certain fields. This option filters which fields
are included in the per-variant metadata output.

Usage:
  - Fields are pipe-separated, like --header-format
  - Only fields that exist in --header-format are included
  - Fields in --metadata-fields not in --header-format are ignored

Example:
  --header-format "country|date|patient|lab|batch"
  --metadata-fields "country|date"

Result: Only "country" and "date" are aggregated per variant.
        "patient", "lab", and "batch" are parsed but not stored.

Benefits:
  - Reduced memory usage (fewer HashMap entries)
  - Smaller output files
  - Faster processing

Note: This does NOT affect header parsing - all fields are still
validated. It only controls which fields appear in the output."#;

    // -------------------------------------------------------------------------
    // Header Fill NA (--header-fillna)
    // -------------------------------------------------------------------------
    pub const HEADER_FILLNA_HELP: &str = "Replacement value for empty header fields";

    pub const HEADER_FILLNA_LONG_HELP: &str = r#"Replacement value for empty or missing header fields.

When FASTA headers have empty fields (consecutive pipes like "USA||Patient001"
or trailing pipes like "USA|2023|"), this value is substituted.

How it works:
  - Applied during header parsing (before validation)
  - Replaces empty strings and whitespace-only fields
  - Uses string interning for memory efficiency

Examples:
  Header: ">USA||Patient001"
  With --header-fillna "Unknown": country="USA", date="Unknown", patient="Patient001"
  With --header-fillna "N/A": country="USA", date="N/A", patient="Patient001"

Common values:
  - "Unknown" (default): Clear indication of missing data
  - "N/A" or "NA": Standard notation for not available
  - "Missing": Explicit missing marker

Note: Empty header fields after trimming whitespace are always replaced.
If you need to keep empty values, this is not currently supported."#;

    // -------------------------------------------------------------------------
    // Alphabet (--alphabet)
    // -------------------------------------------------------------------------
    pub const ALPHABET_HELP: &str = "Sequence type: protein or nucleotide";

    pub const ALPHABET_LONG_HELP: &str = r#"Specify the sequence alphabet type for validation and encoding.

DiMA supports two alphabet types, each with different valid characters
and encoding schemes:

PROTEIN (default):
  Valid characters: A C D E F G H I K L M N P Q R S T V W Y (20 amino acids)
  Ambiguous codes:  X (any), B (D/N), J (L/I), Z (E/Q), O (pyrrolysine), U (selenocysteine)
  K-mer encoding:   Base-20 arithmetic (max k ~ 13-14)
  Use for:          Protein sequence diversity analysis

NUCLEOTIDE:
  Valid characters: A C G T U (DNA + RNA bases)
  Ambiguous codes:  R Y K M S W B D H V N (IUPAC ambiguity codes)
  K-mer encoding:   Base-5 arithmetic (max k ~ 27-28)
  Use for:          DNA/RNA sequence diversity analysis

The alphabet choice affects:
  1. Character validation (what's considered valid/invalid)
  2. K-mer encoding efficiency (base used for numeric encoding)
  3. Maximum practical k-mer length (due to u64 overflow)

Examples:
  --alphabet protein     # Analyze protein sequences (default)
  --alphabet nucleotide  # Analyze DNA/RNA sequences"#;

    // -------------------------------------------------------------------------
    // Output File (-o, --output)
    // -------------------------------------------------------------------------
    pub const OUTPUT_HELP: &str = "Output file path (JSON or binary with --binary)";

    pub const OUTPUT_LONG_HELP: &str = r#"Output file path for analysis results.

If not provided, results are printed to stdout (JSON format only).

Output formats:
  - JSON (default): Human-readable, larger file size
  - Binary (--binary): Compact .dima format, faster I/O

File extensions:
  - Without --binary: .json recommended (but any extension works)
  - With --binary: automatically changed to .dima

Examples:
  -o results.json           # JSON output
  -o results --binary       # Creates results.dima
  -o results.json --binary  # Creates results.dima (extension replaced)

Note: Binary format requires -o/--output (cannot stream to stdout)."#;

    // -------------------------------------------------------------------------
    // HCS Mode (--hcs)
    // -------------------------------------------------------------------------
    pub const HCS_HELP: &str = "Output only Highly Conserved Sequences (Index variants)";

    pub const HCS_LONG_HELP: &str = r#"Output only Highly Conserved Sequences instead of full analysis.

HCS (Highly Conserved Sequences) are regions where the same k-mer appears
most frequently across all sequences. These are extracted from variants
classified as "Index" (motif_short = "I").

How Index variants are classified:
  - Must have the highest count at that position
  - Count must be > 1 (not unique)
  - Represents the "consensus" or most conserved sequence

HCS extraction algorithm:
  1. Find all Index variants at each position
  2. Concatenate overlapping k-mers (suffix-prefix matching)
  3. If no overlap, start a new conserved region
  4. Output as JSON array of sequence strings

Example output:
  ["ACDEFGHIKLMNPQRSTVWY", "FGHIKLMNPQRS"]

Use cases:
  - Identify conserved epitopes for vaccine design
  - Find evolutionarily stable protein regions
  - Extract consensus sequences from alignments

Combine with --hcs-threshold to filter by conservation level."#;

    // -------------------------------------------------------------------------
    // HCS Threshold (--hcs-threshold)
    // -------------------------------------------------------------------------
    pub const HCS_THRESHOLD_HELP: &str = "Minimum incidence percentage (0-100) for HCS variants";

    pub const HCS_THRESHOLD_LONG_HELP: &str = r#"Filter Highly Conserved Sequences by minimum incidence percentage.

Incidence is calculated as:
  (variant.count / total_sequences_at_position) × 100

Range: 0.0 to 100.0 (percentage)

How it works:
  - Only Index variants with incidence >= threshold are included
  - Lower threshold = more sequences (less stringent conservation)
  - Higher threshold = fewer sequences (more stringent conservation)

Examples:
  --hcs --hcs-threshold 95   # Variants in ≥95% of sequences
  --hcs --hcs-threshold 80   # Variants in ≥80% of sequences
  --hcs                      # All Index variants (no threshold)

Recommended values:
  - 95-100: Highly conserved regions (vaccine targets)
  - 80-95:  Moderately conserved regions
  - 50-80:  Semi-conserved regions
  - <50:    Not typically useful (too variable)

Note: Only effective when --hcs is specified."#;

    // -------------------------------------------------------------------------
    // No Metadata (--no-metadata)
    // -------------------------------------------------------------------------
    pub const NO_METADATA_HELP: &str = "Skip header parsing and metadata aggregation (faster)";

    pub const NO_METADATA_LONG_HELP: &str = r#"Disable all header parsing and per-variant metadata aggregation.

When enabled:
  - FASTA headers are ignored (only sequences are processed)
  - No metadata aggregation per variant
  - Variants have "metadata": null in output

Performance benefits:
  - Faster processing (no header parsing overhead)
  - Lower memory usage (no metadata HashMaps)
  - Smaller output files

Use when:
  - You only need diversity/entropy analysis
  - Headers don't contain useful metadata
  - Processing very large datasets where speed matters
  - Headers don't follow a consistent pipe-separated format

Note: This flag overrides --header-format and --metadata-fields."#;

    // -------------------------------------------------------------------------
    // Threads (--threads)
    // -------------------------------------------------------------------------
    pub const THREADS_HELP: &str = "Number of parallel threads (default: all CPUs)";

    pub const THREADS_LONG_HELP: &str = r#"Number of parallel worker threads for computation.

By default, DiMA uses all available CPU cores via Rayon's global thread pool.
This option allows limiting parallelism for resource management.

Parallelized operations:
  - Entropy calculation (per position)
  - K-mer counting and encoding (per position)
  - Metadata aggregation (for datasets with >1000 indices)
  - Index building (when --columnar is enabled)

Guidelines:
  - Default (all CPUs): Best for dedicated analysis
  - --threads 1: Sequential processing (debugging, profiling)
  - --threads N: Limit to N cores (shared systems, containers)

Examples:
  --threads 4    # Use 4 threads
  --threads 1    # Single-threaded (useful for debugging)

Note: Thread pool is initialized once at startup. Overhead is minimal
for large datasets but may dominate for very small files."#;

    // -------------------------------------------------------------------------
    // Columnar Storage (--columnar)
    // -------------------------------------------------------------------------
    pub const COLUMNAR_HELP: &str = "Use columnar metadata storage for better performance";

    pub const COLUMNAR_LONG_HELP: &str = r#"Enable columnar metadata storage for improved performance.

Instead of storing metadata as row-oriented HashMaps (one per sequence),
columnar storage organizes data by field (one vector per field).

Memory layout comparison:
  Row storage:    [Seq1{field1,field2,...}, Seq2{field1,field2,...}, ...]
  Columnar:       [Field1:[val1,val2,...], Field2:[val1,val2,...], ...]

Performance benefits:
  - 20-30% better cache locality (sequential field access)
  - 15-25% memory reduction (Arc<str> string interning)
  - 40-60% faster bulk operations (SIMD-friendly layout)
  - Automatic parallel aggregation for large index sets

Best for:
  - Large datasets (>1000 sequences)
  - Many metadata fields
  - Repeated metadata values (benefits from interning)
  - Frequent metadata aggregation queries

Not recommended for:
  - Small datasets (<100 sequences) - overhead may exceed benefit
  - Single-pass processing without aggregation

Note: When enabled, --indexing is automatically activated for
additional lookup performance (80-95% faster value lookups)."#;

    // -------------------------------------------------------------------------
    // Indexing (--indexing)
    // -------------------------------------------------------------------------
    pub const INDEXING_HELP: &str = "Enable metadata indexing for 80-95% faster lookups";

    pub const INDEXING_LONG_HELP: &str = r#"Enable inverted index construction for metadata fields.

Creates bitmap-based inverted indices mapping field values to sequence
indices, enabling O(1) lookups instead of O(n) linear scans.

Index structure:
  field_value → bitmap(sequence_indices)
  Example: "USA" → [1,0,1,1,0,0,1,...] (64-bit packed)

Performance improvements:
  - 80-95% faster value lookups
  - Efficient set operations (AND/OR via bitmap intersection/union)
  - Pre-computed value counts

Memory overhead:
  - ~64 bits per sequence per unique value
  - Additional cache for composite queries

Best for:
  - Many metadata lookup operations
  - Large datasets (>10,000 sequences)
  - High-cardinality fields (many unique values)
  - Complex multi-field filtering

Note: Currently auto-enabled when --columnar is specified.
This flag is for future independent control of indexing."#;

    // -------------------------------------------------------------------------
    // Binary Format (--binary)
    // -------------------------------------------------------------------------
    pub const BINARY_HELP: &str = "Use binary format (.dima) for 50-70% faster I/O";

    pub const BINARY_LONG_HELP: &str = r#"Output results in binary format instead of JSON.

The .dima binary format provides significant performance and size benefits
over JSON for large analysis results.

File structure:
  - 17-byte header: magic "DIMA", version, compression type, flags
  - String table: deduplicated strings with u32 references
  - Compressed data: bincode-serialized results

Benefits over JSON:
  - 50-70% faster read/write operations
  - 30-50% smaller file size (with compression)
  - String interning eliminates duplicate storage
  - Compact numeric encoding (no text conversion)

Compression options (--compression):
  - 0 (none): Fastest, largest files
  - 1 (LZ4):  Default, fast compression, 20-40% reduction
  - 2 (Zstd): Best compression, 30-50% reduction

Reading binary files:
  Use the 'deflate' subcommand to convert back to JSON:
  dima deflate -i results.dima -o results.json

Requirements:
  - Must specify -o/--output (cannot stream binary to stdout)
  - Output file extension is automatically changed to .dima"#;

    // -------------------------------------------------------------------------
    // Compression Level (--compression)
    // -------------------------------------------------------------------------
    pub const COMPRESSION_HELP: &str = "Binary compression: 0=none, 1=lz4, 2=zstd";

    pub const COMPRESSION_LONG_HELP: &str = r#"Compression level for binary output format.

Only effective when --binary is specified.

Compression levels:

  Level 0 - None:
    - No compression applied
    - Fastest write/read speed
    - Largest file size
    - Use for: real-time processing, temporary files

  Level 1 - LZ4 (default):
    - Fast compression algorithm
    - Compression: ~500 MB/s
    - Decompression: ~2-3 GB/s
    - Size reduction: 20-40%
    - Use for: general purpose, interactive workflows

  Level 2 - Zstd:
    - Higher compression ratio
    - Compression: ~200-400 MB/s
    - Decompression: ~1-2 GB/s
    - Size reduction: 30-50%
    - Use for: archival, network transfer, storage-constrained

Examples:
  --binary --compression 0  # No compression (fastest)
  --binary --compression 1  # LZ4 compression (default)
  --binary --compression 2  # Zstd compression (smallest)"#;

    // -------------------------------------------------------------------------
    // Validation Mode (--validation)
    // -------------------------------------------------------------------------
    pub const VALIDATION_HELP: &str = "Character validation mode: strict, permissive, or report";

    pub const VALIDATION_LONG_HELP: &str = r#"Character validation mode for k-mer generation.

Controls how invalid and ambiguous characters in sequences are handled.

STRICT (default) - Recommended for scientific accuracy:
  Valid characters only. Any other character invalidates the k-mer.
  
  Protein whitelist:    A C D E F G H I K L M N P Q R S T V W Y
                        (20 canonical amino acids)
  Nucleotide whitelist: A C G T U
                        (DNA + RNA bases)
  
  Ambiguous codes (X, B, N, etc.) → k-mer marked as NA
  Invalid chars (#, *, @, numbers) → k-mer marked as NA

PERMISSIVE - Accept ambiguous IUPAC codes:
  Valid + ambiguous characters accepted. Only completely invalid
  characters cause k-mer invalidation.
  
  Protein ambiguous:    X (any), B (D/N), J (L/I), Z (E/Q), O, U
  Nucleotide ambiguous: R Y K M S W B D H V N (IUPAC codes)
  
  Use when: sequences contain standard ambiguity notation

REPORT - Data quality assessment:
  All characters accepted (no k-mer invalidation).
  Statistics are tracked for reporting with --report-invalid.
  
  Use when: exploring unknown data quality

Examples:
  --validation strict      # Strictest, recommended (default)
  --validation permissive  # Allow IUPAC ambiguity codes
  --validation report      # Accept all, track statistics"#;

    // -------------------------------------------------------------------------
    // Allow Lowercase (--allow-lowercase)
    // -------------------------------------------------------------------------
    pub const ALLOW_LOWERCASE_HELP: &str = "Convert lowercase letters to uppercase (default: treat as invalid)";

    pub const ALLOW_LOWERCASE_LONG_HELP: &str = r#"Allow and convert lowercase characters in sequences.

By default, lowercase letters (a-z) are treated as invalid characters
and cause k-mers containing them to be marked as NA.

When enabled:
  - Lowercase letters are converted to uppercase during encoding
  - 'a' → 'A', 'c' → 'C', etc.
  - Conversion happens at the encoding lookup table level (efficient)
  - No performance penalty after initialization

When disabled (default):
  - Lowercase letters are classified as Invalid
  - K-mers containing lowercase are marked as NA
  - Ensures input data quality (catches unexpected formatting)

Use cases for enabling:
  - Input files use mixed case (common in some databases)
  - Sequences use lowercase for masking (soft-masked regions)
  - Converting legacy data with inconsistent formatting

Examples:
  Sequence: "AcDeFGhiK" (mixed case)
  
  Without --allow-lowercase:
    K-mers containing c, h, i marked as NA
  
  With --allow-lowercase:
    Treated as "ACDEFGHIK" (all valid)"#;

    // -------------------------------------------------------------------------
    // Report Invalid (--report-invalid)
    // -------------------------------------------------------------------------
    pub const REPORT_INVALID_HELP: &str = "Print statistics about character validation to stderr";

    pub const REPORT_INVALID_LONG_HELP: &str = r#"Report statistics about character validation after processing.

Prints a summary to stderr showing counts and percentages of character
types encountered during sequence processing.

Statistics tracked:
  - Total characters:     All characters processed
  - Valid characters:     Canonical alphabet characters
  - Ambiguous characters: Known ambiguity codes (X, B, N, etc.)
  - Gap characters:       Alignment gaps (-)
  - Invalid characters:   Unrecognized characters (#, *, @, numbers)
  - Invalidated k-mers:   K-mers marked as NA due to bad characters

Output format:
  Character Validation Summary:
    Total characters:     1,234,567
    Valid characters:     1,200,000 (97.2%)
    Ambiguous characters:    30,000 (2.4%)
    Gap characters:           4,000 (0.3%)
    Invalid characters:         567 (0.05%)
    Invalidated k-mers:       1,234

If invalid characters are found, a warning message is also displayed
identifying the alphabet type and explaining the impact.

Use cases:
  - Data quality assessment before production analysis
  - Debugging unexpected NA k-mers
  - Validating input file formatting"#;
}

// =============================================================================
// DEFLATE COMMAND
// =============================================================================

pub mod deflate {
    // -------------------------------------------------------------------------
    // Input File (-i, --input)
    // -------------------------------------------------------------------------
    pub const INPUT_HELP: &str = "Path to the binary (.dima) file to decompress";

    pub const INPUT_LONG_HELP: &str = r#"Path to the binary .dima file to convert back to JSON.

The .dima format is DiMA's compact binary format created with --binary.
This command reads the binary file and outputs human-readable JSON.

Requirements:
  - File must be a valid .dima binary format
  - File must exist and be readable

The command will warn if the file extension is not .dima, but will
still attempt to read it as binary format."#;

    // -------------------------------------------------------------------------
    // Output File (-o, --output)
    // -------------------------------------------------------------------------
    pub const OUTPUT_HELP: &str = "Output JSON file path (prints to stdout if not specified)";

    pub const OUTPUT_LONG_HELP: &str = r#"Output path for the converted JSON file.

If not specified, JSON is printed to stdout.

When an output file is specified, the command also displays:
  - Source and destination file paths
  - File size comparison (binary vs JSON)
  - Expansion ratio

Example output:
  Binary file 'results.dima' successfully converted to JSON: 'results.json'
  File size comparison:
    Binary (.dima): 1,234,567 bytes
    JSON:           3,456,789 bytes
    Expansion ratio: 2.80x"#;

    // -------------------------------------------------------------------------
    // No Pretty (--no-pretty)
    // -------------------------------------------------------------------------
    pub const NO_PRETTY_HELP: &str = "Output compact JSON without formatting";

    pub const NO_PRETTY_LONG_HELP: &str = r#"Disable pretty-printing for JSON output.

By default, JSON output is formatted with indentation and newlines
for human readability. This option outputs compact JSON on a single
line (or minimal lines).

Pretty output (default):
  {
    "query_name": "Sample",
    "sequence_count": 100,
    "results": [
      ...
    ]
  }

Compact output (--no-pretty):
  {"query_name":"Sample","sequence_count":100,"results":[...]}

Benefits of compact output:
  - Smaller file size (no whitespace)
  - Faster parsing by some JSON libraries
  - Better for machine-to-machine transfer

Use when:
  - Output will be processed by another program
  - Minimizing file size is important
  - Pretty formatting is not needed"#;
}
