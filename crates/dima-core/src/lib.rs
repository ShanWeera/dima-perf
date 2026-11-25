//! DiMA core library: the sequence diversity dynamics analyser for viruses.
//!
//! Implements the method of Tharanga S, Ünlü ES, Hu Y, Sjaugi MF, Çelik MA, Hekimoğlu H,
//! Miotto O, Öncel MM, Khan AM. "DiMA: sequence diversity dynamics analyser for viruses".
//! *Brief Bioinform* 2025;26(1):bbae607, doi:10.1093/bib/bbae607.
#![forbid(unsafe_code)]

#[cfg(not(target_pointer_width = "64"))]
compile_error!("DiMA supports 64-bit targets only (counts and offsets are 64-bit; see docs).");

pub mod alphabet;
pub mod analysis;
pub mod citations;
pub mod config;
pub mod control;
pub mod diagnostics;
pub mod engine;
pub mod entropy;
pub mod error;
pub mod hcs;
pub mod header;
pub mod human;
pub mod input;
pub mod methods;
pub mod model;
pub mod normalise;
mod numeric;
pub mod output;
mod pool;
pub mod position;
pub mod provenance;
pub mod run_report;
pub mod similarity;
pub mod stats;
pub mod summary;
pub mod validation;

pub use analysis::{Analysis, analyze};
pub use error::{DimaError, Result};
