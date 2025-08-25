pub mod models;
pub mod kmer;
pub mod entropy;
pub mod io;
pub mod analysis;
pub mod simd_string;
pub mod zero_copy;
pub mod columnar;
pub mod indexing;
pub mod binary;
 
pub use analysis::{get_results_objs, get_results_objs_columnar};
pub use models::{Results, Position, Variant, HighestEntropy}; 