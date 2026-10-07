//! `dima cite` (D89).

use std::io::Write;

use dima_core::methods::{self, CitationFormat, Cited};
use dima_core::output;
use dima_core::{DimaError, Result};

use super::args::{CiteArgs, CiteFormat};

const fn format_of(f: CiteFormat) -> CitationFormat {
    match f {
        CiteFormat::Text => CitationFormat::Text,
        CiteFormat::Bibtex => CitationFormat::Bibtex,
        CiteFormat::Ris => CitationFormat::Ris,
        CiteFormat::Cff => CitationFormat::Cff,
    }
}

/// Runs `dima cite`.
///
/// # Errors
/// Errors reading `--from` or writing to standard output.
pub fn run(a: &CiteArgs) -> Result<()> {
    let mut results = None;
    let mut hcs = None;
    for path in &a.from {
        match output::read_document(path)? {
            output::Document::Results(r) if results.is_none() => results = Some(r),
            output::Document::Hcs(h) if hcs.is_none() => hcs = Some(h),
            _ => {
                return Err(DimaError::usage(
                    "--from takes at most one results file and one conserved-sequences file",
                )
                .with_hint("give the results of one run and, optionally, its --hcs-output file"));
            }
        }
    }
    let cited = Cited {
        results: results.as_deref(),
        hcs: hcs.as_deref(),
    };
    let format = format_of(a.format);
    let mut text = match (methods::methods_text(cited), format) {
        (Some(t), CitationFormat::Text) => t,
        (None, CitationFormat::Text) => format!(
            "If you use DiMA, please cite the publication and the software version you used:\n\n{}",
            methods::references(cited, format)
        ),
        (_, _) => methods::references(cited, format),
    };
    if !text.ends_with('\n') {
        text.push('\n');
    }
    let mut out = std::io::stdout().lock();
    out.write_all(text.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|e| {
            DimaError::io(
                dima_core::error::IoAction::Write,
                dima_core::error::IoTarget::Stdout,
                e,
            )
        })
}
