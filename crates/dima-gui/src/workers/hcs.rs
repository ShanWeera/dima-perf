//! Conserved regions (D86), computed off the interface thread: on a long alignment the
//! Clopper–Pearson bounds of thousands of positions take long enough to stutter a slider
//! (P5). The app debounces edits and runs one job at a time.

use std::sync::Arc;

use dima_core::Result;
use dima_core::hcs::{self, HcsParams};
use dima_core::model::{HcsResults, Results};

use super::{Priority, Worker};

/// The job's result.
pub type Outcome = Result<Arc<HcsResults>>;

/// Starts the computation for `results` with `params`.
#[must_use]
pub fn spawn(
    results: Arc<Results>,
    params: HcsParams,
    ctx: Option<egui::Context>,
) -> Worker<Outcome> {
    Worker::spawn("hcs", Priority::Background, ctx, move |_| {
        hcs::compute(&results, &params).map(Arc::new)
    })
}
