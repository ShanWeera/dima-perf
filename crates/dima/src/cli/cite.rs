//! `dima cite` (D89).

use std::io::Write;
use std::path::Path;

use dima_core::methods::{self, CitationFormat, Cited};
use dima_core::model::{HcsResults, Results};
use dima_core::output;
use dima_core::{DimaError, Result};

use super::args::{CiteArgs, CiteFormat};
use super::ui;

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
/// Errors reading `--from`, conserved sequences of another run than the results, or errors
/// writing to standard output.
pub fn run(a: &CiteArgs) -> Result<()> {
    let mut results = None;
    let mut hcs = None;
    for path in &a.from {
        match output::read_document(path)? {
            output::Document::Results(r) if results.is_none() => results = Some((r, path)),
            output::Document::Hcs(h) if hcs.is_none() => hcs = Some((h, path)),
            _ => {
                return Err(DimaError::usage(
                    "--from takes at most one results file and one conserved-sequences file",
                )
                .with_hint("give the results of one run and, optionally, its --hcs-output file"));
            }
        }
    }
    if let (Some((r, results_path)), Some((h, hcs_path))) = (&results, &hcs) {
        same_run(r, results_path, h, hcs_path)?;
    }
    let cited = Cited {
        results: results.as_ref().map(|(r, _)| r.as_ref()),
        hcs: hcs.as_ref().map(|(h, _)| h.as_ref()),
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

/// A `--from` file as named in messages.
fn shown(path: &Path) -> String {
    if path.as_os_str() == "-" {
        "standard input".to_owned()
    } else {
        format!("'{}'", path.display())
    }
}

/// Refuses conserved sequences computed from other results than `r`: their input (checksum),
/// k or T differ, so one methods text would describe two analyses.
fn same_run(r: &Results, results_path: &Path, h: &HcsResults, hcs_path: &Path) -> Result<()> {
    let mut differ = Vec::new();
    if h.source.input_checksum != r.run.input.checksum {
        differ.push("the input".to_owned());
    }
    if h.source.kmer_length != r.kmer_length {
        differ.push(format!(
            "k ({} and {})",
            r.kmer_length, h.source.kmer_length
        ));
    }
    if h.source.support_threshold != r.support_threshold {
        differ.push(format!(
            "T ({} and {})",
            r.support_threshold, h.source.support_threshold
        ));
    }
    let Some((last, rest)) = differ.split_last() else {
        return Ok(());
    };
    let list = if rest.is_empty() {
        last.clone()
    } else {
        format!("{} and {last}", rest.join(", "))
    };
    let command = match ui::shell_word(results_path) {
        Some(word) if results_path.as_os_str() != "-" => format!("dima hcs -i {word}"),
        _ => "dima hcs -i RESULTS".to_owned(),
    };
    Err(DimaError::usage(format!(
        "the conserved sequences in {} were not computed from the results in {}: {list} {}",
        shown(hcs_path),
        shown(results_path),
        if differ.len() == 1 {
            "differs"
        } else {
            "differ"
        }
    ))
    .with_hint(format!(
        "compute the conserved sequences from these results: {command}"
    )))
}
