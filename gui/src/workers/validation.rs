//! Background FASTA validation.
//!
//! `validate_fasta` streams the whole file (transparently decompressing gzip /
//! bzip2 / xz / zstd), which takes seconds on large viral alignments. Running it
//! off the render thread keeps the window responsive and lets the user cancel by
//! simply choosing a different file.

use std::path::PathBuf;

use super::Worker;

/// What a validation job reports back.
pub type ValidationOutcome = Result<dima_lib::FastaValidationResult, std::io::Error>;

/// Spawn validation of `path` on a background thread.
pub fn spawn(ctx: Option<egui::Context>, path: PathBuf) -> Worker<ValidationOutcome> {
    Worker::spawn(ctx, move |cancel| {
        // The library takes the shared handle itself so it can check the flag
        // from inside its streaming scan loop.
        dima_lib::validate_fasta(&path, Some(&cancel))
    })
}
