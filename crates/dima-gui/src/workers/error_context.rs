//! The context of a failed run for the error card (D85): the header of the offending record
//! and the characters around the offending column, read back from the input after the failure.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dima_core::control::Progress;
use dima_core::input::{self, RawBatch, Source};

use super::{Priority, Worker};

/// What the error card shows about the offending record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecordExcerpt {
    /// The header line (without `>`), cut to a readable length.
    pub header: String,
    /// The sequence around the offending column, the column bracketed: `…KLMN[1]PQRS…`.
    pub around_column: Option<String>,
}

/// Header characters shown.
const HEADER_CHARS: usize = 160;
/// Residues shown on each side of the offending column.
const CONTEXT: usize = 12;

/// Reads `path` up to record `record` (1-based) and extracts its excerpt.
#[must_use]
pub fn excerpt(
    path: &Path,
    record: u64,
    column: Option<u64>,
    cancel: &AtomicBool,
) -> Option<RecordExcerpt> {
    let progress = Arc::new(Progress::new());
    let mut opened = input::open(&Source::Path(path.to_path_buf()), &progress).ok()?;
    let mut batch = RawBatch::default();
    loop {
        let more = opened.batches.read_batch(&mut batch, cancel).ok()?;
        let first = batch.first_record;
        let last = first + batch.len() as u64;
        if (first..last).contains(&record) {
            let i = usize::try_from(record - first).ok()?;
            let header = String::from_utf8_lossy(batch.header(i));
            let around_column = column.and_then(|c| around(batch.sequence(i).0, c));
            return Some(RecordExcerpt {
                header: crate::util::truncate_display(&header, HEADER_CHARS),
                around_column,
            });
        }
        if !more || batch.is_empty() {
            return None;
        }
    }
}

/// The residues around 1-based alignment `column` (whitespace and line breaks skipped, as the
/// library counts columns), with the column itself in brackets.
#[must_use]
pub fn around(sequence: &[u8], column: u64) -> Option<String> {
    let cells: Vec<u8> = sequence
        .iter()
        .copied()
        .filter(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0B | 0x0C))
        .collect();
    let c = usize::try_from(column).ok()?.checked_sub(1)?;
    let byte = *cells.get(c)?;
    let show = |b: &[u8]| {
        b.iter()
            .map(|&x| {
                if x.is_ascii_graphic() {
                    char::from(x).to_string()
                } else {
                    format!("\\x{x:02X}")
                }
            })
            .collect::<String>()
    };
    let start = c.saturating_sub(CONTEXT);
    let end = (c + 1 + CONTEXT).min(cells.len());
    Some(format!(
        "{}{}[{}]{}{}",
        if start > 0 { "\u{2026}" } else { "" },
        show(&cells[start..c]),
        show(&[byte]),
        show(&cells[c + 1..end]),
        if end < cells.len() { "\u{2026}" } else { "" }
    ))
}

/// Starts reading the excerpt of `record` of `path`.
#[must_use]
pub fn spawn(
    path: std::path::PathBuf,
    record: u64,
    column: Option<u64>,
    ctx: Option<egui::Context>,
) -> Worker<Option<RecordExcerpt>> {
    Worker::spawn("error-context", Priority::Normal, ctx, move |cancel| {
        excerpt(&path, record, column, &cancel)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_of_a_record() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.fa");
        std::fs::write(&p, ">first|x\nACDE\n>second|Camel|2014\nAC\nD1\n").unwrap();
        let e = excerpt(&p, 2, Some(4), &AtomicBool::new(false)).unwrap();
        assert_eq!(e.header, "second|Camel|2014");
        assert_eq!(e.around_column.as_deref(), Some("ACD[1]"));
        assert_eq!(excerpt(&p, 9, None, &AtomicBool::new(false)), None);
    }

    #[test]
    fn context_marks_the_column() {
        let seq = b"ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGH\n";
        assert_eq!(
            around(seq, 20).as_deref(),
            Some("\u{2026}HIJKLMNOPQRS[T]UVWXYZABCDEF\u{2026}")
        );
        assert_eq!(around(b"A C\nD", 3).as_deref(), Some("AC[D]"));
        assert_eq!(around(b"AB\x00", 3).as_deref(), Some("AB[\\x00]"));
        assert_eq!(around(b"AB", 0), None);
        assert_eq!(around(b"AB", 5), None);
    }
}
