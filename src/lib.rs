// Many internal functions are used by consumers outside this crate's binary
// (Tauri app, benchmarks) but appear dead from the lib's perspective.
#![allow(dead_code)]

// Internal modules — NOT part of the semver-stable public API.
// Only types explicitly re-exported below are guaranteed stable.
// Using pub(crate) prevents external crates from directly importing
// implementation details, allowing internal refactoring without semver bumps.
pub(crate) mod alphabet;
pub(crate) mod models;
pub mod kmer;
pub(crate) mod entropy;
pub mod io;
pub(crate) mod analysis;
pub(crate) mod simd_string;
pub(crate) mod zero_copy;
pub(crate) mod columnar;
pub(crate) mod indexing;
pub mod binary;
pub mod matrix;
pub mod output;
pub mod perf;

// ─── Stable Public API ───────────────────────────────────────────────────────
// Only these re-exports are semver-guaranteed. Internal module structure may
// change between minor versions without breaking downstream consumers.

pub use alphabet::{CharacterValidator, ValidationMode, AlphabetType, CharacterClass, ValidationStats};
pub use analysis::{analyze, get_results_objs, get_results_objs_columnar, AnalysisConfig, AnalysisError};
pub use entropy::calculate_entropy_encoded_at_position;
pub use kmer::max_kmer_length;
pub use models::{Results, Position, Variant, HighestEntropy};
pub use binary::{BinaryFormat, BinaryFormatConfig, BinaryFormatError, CompressionType};
pub use io::{InputSource, ParseDiagnostics, atomic_write};
pub use output::{OutputType, OutputOptions, resolve_output_type, write_results_to_output};
pub use perf::PerfReport;
