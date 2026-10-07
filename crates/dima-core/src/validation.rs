//! The first-batch probe shared by the analysis and the GUI's pre-check (D40, D85).
//!
//! [`inspect`] reads only the first batch (at least min(1,000, N) records, at most 10,000
//! records or 64 MiB) and reports what a user needs to configure a run — alphabet, alignment
//! length, default k, header fields with sample values, compression, size and an estimated
//! record count — in well under a second. It validates nothing beyond what it reads; the run
//! itself is the validator (fail-fast, with the exact location of the first problem).

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::SystemTime;

use crate::alphabet::{LetterCounts, guess};
use crate::config::Alphabet;
use crate::control::{ProbeFacts, Progress};
use crate::header::{CleanStats, clean_value};
use crate::input::{self, RawBatch, Source};
use crate::model::Compression;
use crate::normalise::count_columns;

/// Letter counts of the sequences in a batch (the alphabet probe).
///
/// Sequential on purpose: the pre-check runs outside any analysis thread pool, where rayon
/// would fall back to its global pool, which ignores the chosen thread count and, once the
/// system refuses one of its threads, stays unusable for the rest of the process. One batch
/// (at most 64 MiB) takes tens of milliseconds as a byte histogram.
#[must_use]
pub fn probe_letters(batch: &RawBatch) -> LetterCounts {
    let mut histogram = [0u64; 256];
    for i in 0..batch.len() {
        for &b in batch.sequence(i).0 {
            histogram[usize::from(b)] += 1;
        }
    }
    LetterCounts::from_histogram(&histogram)
}

/// What the first batch shows (D108): the same facts for the pre-check and the run, from this
/// one function, so that a difference can only mean the input changed in between.
#[must_use]
pub fn probe_facts(first: &RawBatch) -> ProbeFacts {
    ProbeFacts {
        records: first.len() as u64,
        letters: probe_letters(first),
        alignment_length: if first.is_empty() {
            0
        } else {
            count_columns(first.sequence(0).0)
        },
        header_fields: if first.is_empty() {
            0
        } else {
            memchr::memchr_iter(b'|', first.header(0)).count() + 1
        },
    }
}

/// One `|`-separated header field seen in the probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderField {
    /// Up to three distinct cleaned sample values, in input order.
    pub samples: Vec<String>,
    /// `true` when every probed record had a different value (likely an identifier).
    pub identifier_like: bool,
}

/// What the pre-check found.
#[derive(Clone, Debug, PartialEq)]
pub struct Inspection {
    /// Input name for display.
    pub source_name: String,
    /// Compression detected from the content.
    pub compression: Compression,
    /// Stored size (regular files).
    pub size_bytes: Option<u64>,
    /// Modification time (regular files), to detect changes before the run (D85, G65).
    pub modified: Option<SystemTime>,
    /// Records examined.
    pub records_examined: u64,
    /// `true` when the probe read the whole input (the record count is exact).
    pub complete: bool,
    /// Estimated number of records (exact when `complete`), extrapolated from the stored bytes
    /// consumed per record; `None` for streams of unknown size.
    pub estimated_records: Option<u64>,
    /// Alignment columns of the first record.
    pub alignment_length: usize,
    /// Alphabet detected from the probe, if it can be determined.
    pub alphabet: Option<Alphabet>,
    /// Residue letters examined.
    pub residues_examined: u64,
    /// The query name the file name implies.
    pub suggested_query_name: Option<String>,
    /// Fields of the first header (split on `|`).
    pub header_fields: Vec<HeaderField>,
    /// Probed records whose headers have a different number of fields than the first.
    pub header_field_count_mismatches: u64,
    /// The facts a run checks before counting, to refuse a file that changed since (D108).
    pub probe: ProbeFacts,
}

/// Reads the first batch of `source` and describes it (D85).
///
/// # Errors
/// The errors of [`input::open`] and of reading the first batch (I/O, unsupported or corrupt
/// compression, structural FASTA errors).
pub fn inspect(source: &Source) -> crate::Result<Inspection> {
    let progress = Arc::new(Progress::new());
    let mut opened = input::open(source, &progress)?;
    let mut batch = RawBatch::default();
    let cancel = AtomicBool::new(false);
    opened.batches.read_batch(&mut batch, &cancel)?;
    let complete = opened.batches.is_finished();
    let probe = probe_facts(&batch);
    let records = probe.records;
    let estimated_records = if complete {
        Some(records)
    } else {
        estimate_records(&batch, opened.compression, opened.stored_size, &progress)
    };
    let (header_fields, mismatches) = header_fields(&batch);
    Ok(Inspection {
        source_name: source.display_name(),
        compression: opened.compression,
        size_bytes: opened.stored_size,
        modified: opened.modified,
        records_examined: records,
        complete,
        estimated_records,
        alignment_length: probe.alignment_length,
        alphabet: guess(&probe.letters),
        residues_examined: probe.letters.total(),
        suggested_query_name: input::query_name_from_source(source),
        header_fields,
        header_field_count_mismatches: mismatches,
        probe,
    })
}

/// Extrapolates the record count from the first batch: from the text consumed per record for
/// plain files (exact offsets), from the stored bytes read per record for compressed files
/// (approximate: decoders read ahead).
fn estimate_records(
    batch: &RawBatch,
    compression: Compression,
    stored_size: Option<u64>,
    progress: &Progress,
) -> Option<u64> {
    let size = stored_size?;
    let records = batch.len() as u64;
    let last = batch.records.last()?;
    let consumed = if compression == Compression::None {
        last.byte + last.range.len() as u64
    } else {
        progress.snapshot().bytes_read
    };
    if consumed == 0 {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let estimate = (size as f64 * records as f64 / consumed as f64).round() as u64;
    Some(estimate.max(records))
}

fn header_fields(batch: &RawBatch) -> (Vec<HeaderField>, u64) {
    let headers: Vec<String> = (0..batch.len())
        .map(|i| String::from_utf8_lossy(batch.header(i)).into_owned())
        .collect();
    let first: Vec<&str> = headers[0].split('|').collect();
    let mut columns: Vec<Vec<String>> = vec![Vec::new(); first.len()];
    let mut mismatches = 0u64;
    let mut stats = CleanStats::default();
    for h in &headers {
        let parts: Vec<&str> = h.split('|').collect();
        if parts.len() != first.len() {
            mismatches += 1;
            continue;
        }
        for (c, p) in columns.iter_mut().zip(parts) {
            c.push(clean_value(p, "", &mut stats));
        }
    }
    let fields = columns
        .into_iter()
        .map(|values| {
            let mut samples: Vec<String> = Vec::with_capacity(3);
            for v in &values {
                if samples.len() == 3 {
                    break;
                }
                if !samples.contains(v) {
                    samples.push(v.clone());
                }
            }
            let mut distinct: Vec<&String> = values.iter().collect();
            distinct.sort_unstable();
            distinct.dedup();
            HeaderField {
                samples,
                identifier_like: values.len() >= crate::config::ID_DIAG_MIN_RECORDS
                    && distinct.len() == values.len(),
            }
        })
        .collect();
    (fields, mismatches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn inspect_mers_sample() {
        let p = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../samples/mers_spike_aa.fasta"
        ));
        let i = inspect(&Source::Path(p)).unwrap();
        assert_eq!(i.alphabet, Some(Alphabet::Protein));
        assert_eq!(i.alignment_length, 1353);
        assert!(i.complete);
        assert_eq!(i.records_examined, 191);
        assert_eq!(i.estimated_records, Some(191));
        assert_eq!(i.compression, Compression::None);
        assert_eq!(i.suggested_query_name.as_deref(), Some("mers_spike_aa"));
        assert_eq!(i.header_fields.len(), 4);
        assert!((1..=3).contains(&i.header_fields[1].samples.len()));
        assert!(
            i.header_fields[0].identifier_like,
            "accessions are all distinct"
        );
        assert!(!i.header_fields[1].identifier_like);
    }

    #[test]
    fn inspect_estimates_large_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("big.fa");
        let mut f = std::fs::File::create(&p).unwrap();
        for i in 0..25_000 {
            writeln!(f, ">s{i}|x\nACGTACGTAC").unwrap();
        }
        drop(f);
        let i = inspect(&Source::Path(p)).unwrap();
        assert!(!i.complete);
        assert_eq!(i.records_examined, 10_000);
        let est = i.estimated_records.unwrap();
        assert!((20_000..=30_000).contains(&est), "{est}");
        assert_eq!(i.alphabet, Some(Alphabet::Dna));
        assert_eq!(i.alignment_length, 10);
    }

    #[test]
    fn inspect_reports_mismatched_headers_and_undetermined_alphabet() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("h.fa");
        std::fs::write(&p, ">a|b|c\nAC\n>d|e\nAC\n").unwrap();
        let i = inspect(&Source::Path(p)).unwrap();
        assert_eq!(i.header_fields.len(), 3);
        assert_eq!(i.header_field_count_mismatches, 1);
        assert_eq!(i.alphabet, None, "4 letters are not enough");
        assert_eq!(i.residues_examined, 4);
    }
}
