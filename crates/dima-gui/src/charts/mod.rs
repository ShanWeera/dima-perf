//! The entropy chart's model and geometry, independent of the renderer.
//!
//! * `metric`: what is plotted (entropy, or similarity-weighted entropy when computed).
//! * `series`: what is drawn (classes, line runs, no-support bands; D87).
//! * `transform`, `axis`, `viewport`: data ↔ screen mapping, ticks, zoom and pan.
//! * `m4`: per-pixel-column downsampling that keeps every extremum.
//! * `gpu_line`: the solid line on the GPU; `scene`: the same figure as SVG or PNG.

pub mod axis;
pub mod gpu_line;
pub mod m4;
pub mod metric;
pub mod scene;
pub mod series;
pub mod transform;
pub mod viewport;
