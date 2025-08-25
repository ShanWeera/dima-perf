pub mod models;
pub mod kmer;
pub mod entropy;
pub mod io;
pub mod analysis;
pub mod simd_string;
pub mod zero_copy;
 
pub use analysis::get_results_objs;
pub use models::{Results, Position, Variant, HighestEntropy}; 