//! Chart rendering utilities.
//!
//! - `axis` / `lttb`: shared maths (tick generation, downsampling).
//! - `gpu_line`: the on-screen, GPU-accelerated entropy line.
//! - `scene`: an egui-independent description of the same figure, rendered to
//!   SVG or PNG for file export.

pub mod axis;
pub mod gpu_line;
pub mod lttb;
pub mod scene;
