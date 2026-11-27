//! Parallel normalisation of raw records into fixed-width rows of canonical residues.
//!
//! Every sequence byte is classified through the alphabet's [`ByteClassTable`]: residues become
//! their upper-case canonical letter (T/U folded), every other column becomes 0 (it excludes the
//! k-mers that contain it), whitespace is skipped (D37), a NUL byte is a structural error (D38)
//! and unknown characters follow the validation mode (D3). Each record must have exactly as many
//! alignment columns as the first (D36). Records are processed in parallel chunks and the results
//! merged in input order, so statistics, examples and the first error are deterministic.

use rayon::prelude::*;

use crate::alphabet::{ByteClassTable, is_whitespace};
use crate::config::{Alphabet, ValidationMode};
use crate::diagnostics::Tally;
use crate::error::{DimaError, Location, Result};
use crate::header::{HeaderPosition, HeaderSpec, HeaderStats, parse_header};
use crate::input::RawBatch;
use crate::model::ByteClass;

/// Records per parallel task.
const CHUNK_RECORDS: usize = 64;

/// Number of alignment columns in a sequence: every byte except line feeds and whitespace.
#[must_use]
pub fn count_columns(seq: &[u8]) -> usize {
    seq.iter()
        .filter(|&&b| b != b'\n' && !is_whitespace(b))
        .count()
}

/// Character and whitespace statistics of sequence lines, mergeable in input order.
#[derive(Clone, Debug)]
pub struct SequenceStats {
    /// Occurrences of each byte value in sequence lines (line endings excluded).
    pub characters: Box<[u64; 256]>,
    /// Lower-case letters folded to upper case.
    pub lowercase_folded: u64,
    /// T (RNA) or U (DNA) folded to the alphabet's letter.
    pub t_u_folded: u64,
    /// Lines with trailing whitespace.
    pub trailing_whitespace_lines: u64,
    /// Lines with whitespace before a residue, with examples.
    pub interior_whitespace: Tally,
    /// Characters of class unknown (permissive/report modes), with examples.
    pub unknown: Tally,
}

impl Default for SequenceStats {
    fn default() -> Self {
        Self {
            characters: Box::new([0; 256]),
            lowercase_folded: 0,
            t_u_folded: 0,
            trailing_whitespace_lines: 0,
            interior_whitespace: Tally::default(),
            unknown: Tally::default(),
        }
    }
}

impl SequenceStats {
    /// Adds statistics of later input.
    pub fn merge(&mut self, other: Self) {
        for (a, b) in self.characters.iter_mut().zip(other.characters.iter()) {
            *a += b;
        }
        self.lowercase_folded += other.lowercase_folded;
        self.t_u_folded += other.t_u_folded;
        self.trailing_whitespace_lines += other.trailing_whitespace_lines;
        self.interior_whitespace.merge(other.interior_whitespace);
        self.unknown.merge(other.unknown);
    }
}

/// Printable description of a byte for messages: `'J' (0x4A)` or `0xC3`.
#[must_use]
pub fn describe_byte(b: u8) -> String {
    if b.is_ascii_graphic() {
        format!("'{}' (0x{b:02X})", char::from(b))
    } else {
        format!("0x{b:02X}")
    }
}

/// The hint for a character outside the alphabet in strict mode. A wrong `--alphabet` is the
/// likely cause when the residues of the first records look like another alphabet, or when a
/// nucleotide alignment holds a letter only protein sequences use, so the hint says so before it
/// offers permissive mode (which would exclude every k-mer with such a letter).
fn unknown_character_hint(b: u8, alphabet: Alphabet, looks_like: Option<Alphabet>) -> String {
    const PERMISSIVE: &str = "use --validation permissive to exclude the k-mers that contain \
                              such characters (they are counted and reported)";
    if let Some(other) = looks_like {
        return format!(
            "check --alphabet first: the residues of the first records look like {other}, not \
             {alphabet}; or {PERMISSIVE}"
        );
    }
    let protein = ByteClassTable::new(Alphabet::Protein);
    if alphabet.is_nucleotide()
        && b.is_ascii_alphabetic()
        && matches!(protein.class(b), ByteClass::Residue | ByteClass::Ambiguity)
    {
        return format!(
            "check --alphabet first: '{}' is a protein letter, not a nucleotide one; or \
             {PERMISSIVE}",
            char::from(b)
        );
    }
    format!("check --alphabet, or {PERMISSIVE}")
}

/// The result of normalising one batch.
#[derive(Debug, Default)]
pub struct NormalisedBatch {
    /// `records × width` canonical bytes (0 = excluded column).
    pub rows: Vec<u8>,
    /// Number of records.
    pub records: usize,
    /// Cleaned values of the stratified header fields, per record (empty without a format).
    pub header_values: Vec<Vec<String>>,
    /// Sequence statistics.
    pub sequence_stats: SequenceStats,
    /// Header statistics.
    pub header_stats: HeaderStats,
}

/// Normalises batches for one alphabet, alignment width and header format.
#[derive(Clone, Copy, Debug)]
pub struct Normaliser<'a> {
    /// Symbol table of the resolved alphabet.
    pub table: &'a ByteClassTable,
    /// Validation mode.
    pub mode: ValidationMode,
    /// Alignment width *L* (columns of the first record).
    pub width: usize,
    /// Header format, if metadata are parsed.
    pub header: Option<&'a HeaderSpec>,
    /// Input name for messages.
    pub source: &'a str,
    /// The alphabet the residues of the first records look like when it is not the table's
    /// (an explicit `--alphabet` that the data contradict; [`crate::alphabet::Resolution::looks_like`]).
    pub looks_like: Option<Alphabet>,
}

struct ChunkOut {
    header_values: Vec<Vec<String>>,
    sequence_stats: SequenceStats,
    header_stats: HeaderStats,
    error: Option<DimaError>,
}

impl Normaliser<'_> {
    /// Normalises `batch` into `out` (reusing its allocations). Runs on the current rayon pool.
    ///
    /// # Errors
    /// The first error in input order: invalid UTF-8 in a header (strict), a NUL byte, an
    /// unknown character (strict), or a record whose length differs from the alignment width.
    pub fn normalise(&self, batch: &RawBatch, out: &mut NormalisedBatch) -> Result<()> {
        let n = batch.len();
        out.records = n;
        out.rows.clear();
        out.rows.resize(n * self.width, 0);
        out.header_values.clear();
        out.sequence_stats = SequenceStats::default();
        out.header_stats = HeaderStats::default();
        if n == 0 {
            return Ok(());
        }
        let chunk_bytes = (CHUNK_RECORDS * self.width).max(1);
        let chunks: Vec<ChunkOut> = if self.width == 0 {
            // Unreachable for valid input (the first record defines a positive width), but keep
            // the record-level error path rather than dividing rows by zero.
            vec![self.normalise_chunk(batch, 0, &mut [])]
        } else {
            out.rows
                .par_chunks_mut(chunk_bytes)
                .enumerate()
                .map(|(c, rows)| self.normalise_chunk(batch, c * CHUNK_RECORDS, rows))
                .collect()
        };
        for chunk in chunks {
            if let Some(e) = chunk.error {
                return Err(e);
            }
            out.header_values.extend(chunk.header_values);
            out.sequence_stats.merge(chunk.sequence_stats);
            out.header_stats.merge(chunk.header_stats);
        }
        Ok(())
    }

    fn normalise_chunk(&self, batch: &RawBatch, first: usize, rows: &mut [u8]) -> ChunkOut {
        let mut out = ChunkOut {
            header_values: Vec::new(),
            sequence_stats: SequenceStats::default(),
            header_stats: HeaderStats::default(),
            error: None,
        };
        let count = rows
            .len()
            .checked_div(self.width)
            .unwrap_or(batch.len() - first);
        for j in 0..count {
            let i = first + j;
            let r = &batch.records[i];
            let record_no = batch.first_record + i as u64;
            if let Some(spec) = self.header {
                let at = HeaderPosition {
                    source: self.source,
                    record: record_no,
                    line: r.line,
                    byte: r.byte,
                };
                match parse_header(batch.header(i), spec, self.mode, at, &mut out.header_stats) {
                    Ok(v) => out.header_values.push(v),
                    Err(e) => {
                        out.error = Some(e);
                        return out;
                    }
                }
            }
            let (seq, offset) = batch.sequence(i);
            let row = if self.width == 0 {
                &mut [][..]
            } else {
                &mut rows[j * self.width..(j + 1) * self.width]
            };
            let start = SeqStart {
                record: record_no,
                line: r.line + 1,
                byte: r.byte + offset as u64,
            };
            if let Err(e) = self.normalise_record(seq, row, start, &mut out.sequence_stats) {
                out.error = Some(e);
                return out;
            }
        }
        out
    }

    fn location(&self, start: SeqStart, line: u64, column: u64, byte: u64) -> Location {
        Location::new(self.source)
            .record(start.record)
            .line(line)
            .column(column)
            .byte(byte)
    }

    fn normalise_record(
        &self,
        seq: &[u8],
        row: &mut [u8],
        start: SeqStart,
        st: &mut SequenceStats,
    ) -> Result<()> {
        let strict = self.mode == ValidationMode::Strict;
        let mut col = 0usize;
        let mut line = start.line;
        let mut pending_ws = false;
        let mut interior_on_line = false;
        let mut i = 0usize;
        while i < seq.len() {
            let b = seq[i];
            if b == b'\n' || (b == b'\r' && seq.get(i + 1) == Some(&b'\n')) {
                if b == b'\r' {
                    i += 1; // CR LF: the CR is part of the line ending.
                }
                if pending_ws {
                    st.trailing_whitespace_lines += 1;
                }
                if interior_on_line {
                    let (r, l) = (start.record, line);
                    st.interior_whitespace
                        .add(1, || format!("record {r}, line {l}"));
                }
                pending_ws = false;
                interior_on_line = false;
                line += 1;
                i += 1;
                continue;
            }
            if b == 0 {
                return Err(DimaError::input_format(
                    self.location(start, line, col as u64 + 1, start.byte + i as u64),
                    "NUL byte in a sequence",
                )
                .with_hint("the file is binary or corrupt; FASTA is plain text"));
            }
            st.characters[usize::from(b)] += 1;
            let class = self.table.class(b);
            if class == ByteClass::Whitespace {
                pending_ws = true;
                i += 1;
                continue;
            }
            if pending_ws {
                interior_on_line = true;
                pending_ws = false;
            }
            let canonical = match class {
                ByteClass::Unknown => {
                    if strict {
                        return Err(DimaError::input_data(
                            Some(self.location(start, line, col as u64 + 1, start.byte + i as u64)),
                            format!(
                                "character {} is not valid in a {} alignment",
                                describe_byte(b),
                                self.table.alphabet()
                            ),
                        )
                        .with_hint(unknown_character_hint(
                            b,
                            self.table.alphabet(),
                            self.looks_like,
                        )));
                    }
                    let (r, l, c) = (start.record, line, col + 1);
                    st.unknown.add(1, || {
                        format!("{} at record {r}, line {l}, column {c}", describe_byte(b))
                    });
                    0
                }
                ByteClass::Residue => {
                    if b.is_ascii_lowercase() {
                        st.lowercase_folded += 1;
                    }
                    if self.table.is_folded_t_u(b) {
                        st.t_u_folded += 1;
                    }
                    self.table.canonical(b)
                }
                _ => {
                    if b.is_ascii_lowercase() {
                        st.lowercase_folded += 1;
                    }
                    0
                }
            };
            if let Some(slot) = row.get_mut(col) {
                *slot = canonical;
            }
            col += 1;
            i += 1;
        }
        if pending_ws {
            st.trailing_whitespace_lines += 1;
        }
        if col == 0 {
            return Err(empty_sequence(self.source, start.record, start.line - 1));
        }
        if col != self.width {
            return Err(DimaError::input_data(
                Some(
                    Location::new(self.source)
                        .record(start.record)
                        .line(start.line - 1),
                ),
                format!(
                    "the sequence has {col} alignment columns, but the first record has {}",
                    self.width
                ),
            )
            .with_hint(
                "DiMA needs a multiple sequence alignment: every sequence must have the same \
                 length (align them first, e.g. with MAFFT or Clustal Omega)",
            ));
        }
        Ok(())
    }
}

/// A record without sequence data (its header is on `header_line`).
pub(crate) fn empty_sequence(source: &str, record: u64, header_line: u64) -> DimaError {
    DimaError::input_data(
        Some(Location::new(source).record(record).line(header_line)),
        "the sequence is empty",
    )
    .with_hint(
        "every header line must be followed by its aligned sequence; remove records without \
         one",
    )
}

#[derive(Clone, Copy, Debug)]
struct SeqStart {
    record: u64,
    line: u64,
    byte: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Alphabet;
    use crate::input::RawRecord;

    fn batch(text: &[u8]) -> RawBatch {
        // Minimal splitter for tests: records start at '>' at line start.
        let mut b = RawBatch {
            first_record: 1,
            ..RawBatch::default()
        };
        b.data = text.to_vec();
        let mut starts: Vec<usize> = vec![];
        for (i, &c) in text.iter().enumerate() {
            if c == b'>' && (i == 0 || text[i - 1] == b'\n') {
                starts.push(i);
            }
        }
        let mut line = 1u64;
        for (k, &s) in starts.iter().enumerate() {
            let e = starts.get(k + 1).copied().unwrap_or(text.len());
            let header_end = text[s..e].iter().position(|&c| c == b'\n').unwrap();
            b.records.push(RawRecord {
                range: s..e,
                header_end,
                line,
                byte: s as u64,
            });
            line += text[s..e].iter().filter(|&&c| c == b'\n').count() as u64;
        }
        b
    }

    fn run(
        text: &[u8],
        alphabet: Alphabet,
        mode: ValidationMode,
        header: Option<&HeaderSpec>,
    ) -> Result<NormalisedBatch> {
        let table = ByteClassTable::new(alphabet);
        let b = batch(text);
        let width = count_columns(b.sequence(0).0);
        let n = Normaliser {
            table: &table,
            mode,
            width,
            header,
            source: "t.fa",
            looks_like: None,
        };
        let mut out = NormalisedBatch::default();
        n.normalise(&b, &mut out)?;
        Ok(out)
    }

    #[test]
    fn rows_are_canonical_and_wrapping_invariant() {
        let a = run(
            b">a\nACGT-N\n>b\nacgu~?\n",
            Alphabet::Dna,
            ValidationMode::Strict,
            None,
        )
        .unwrap();
        assert_eq!(a.rows, b"ACGT\0\0ACGT\0\0");
        assert_eq!(a.sequence_stats.lowercase_folded, 4);
        assert_eq!(a.sequence_stats.t_u_folded, 1);
        // Wrapped, CRLF, trailing and interior whitespace: same rows.
        let b = run(
            b">a\r\nAC\r\nGT-N  \r\n>b\nac gu\n~?\t\n",
            Alphabet::Dna,
            ValidationMode::Strict,
            None,
        )
        .unwrap();
        assert_eq!(b.rows, a.rows);
        assert_eq!(b.sequence_stats.trailing_whitespace_lines, 2);
        assert_eq!(b.sequence_stats.interior_whitespace.count, 1);
        assert_eq!(
            b.sequence_stats.interior_whitespace.examples,
            ["record 2, line 5"]
        );
        assert_eq!(b.sequence_stats.characters[usize::from(b' ')], 3);
        assert_eq!(
            b.sequence_stats.characters[usize::from(b'\r')],
            0,
            "CR LF is a line ending"
        );
        // RNA folds T to U; protein keeps U and O as residues and excludes X, *, #.
        let r = run(b">a\nACGT\n", Alphabet::Rna, ValidationMode::Strict, None).unwrap();
        assert_eq!(r.rows, b"ACGU");
        let p = run(
            b">a\nMUOX*#$^\n",
            Alphabet::Protein,
            ValidationMode::Strict,
            None,
        )
        .unwrap();
        assert_eq!(p.rows, b"MUO\0\0\0\0\0");
    }

    #[test]
    fn strict_and_permissive_unknown_characters() {
        let e = run(
            b">a\nACGT\n>b\nACJT\n",
            Alphabet::Dna,
            ValidationMode::Strict,
            None,
        )
        .unwrap_err();
        assert_eq!(
            e.to_string(),
            "t.fa: record 2 (line 4, column 3, byte 13): character 'J' (0x4A) is not valid in a dna alignment"
        );
        assert_eq!(e.class(), crate::error::ExitClass::InvalidInput);
        // J is a protein letter: the hint points at --alphabet before permissive mode.
        assert_eq!(
            e.hint(),
            Some(
                "check --alphabet first: 'J' is a protein letter, not a nucleotide one; or use \
                 --validation permissive to exclude the k-mers that contain such characters \
                 (they are counted and reported)"
            )
        );
        // A character no alphabet uses keeps the general hint.
        let e = run(
            b">a\nACGT\n>b\nAC1T\n",
            Alphabet::Dna,
            ValidationMode::Strict,
            None,
        )
        .unwrap_err();
        assert!(
            e.hint()
                .unwrap()
                .starts_with("check --alphabet, or use --validation permissive"),
            "{e:?}"
        );
        // When the residues look like another alphabet (an explicit --alphabet the data
        // contradict), the hint says so.
        assert_eq!(
            unknown_character_hint(b'F', Alphabet::Dna, Some(Alphabet::Protein)),
            "check --alphabet first: the residues of the first records look like protein, not \
             dna; or use --validation permissive to exclude the k-mers that contain such \
             characters (they are counted and reported)"
        );
        let p = run(
            b">a\nACGT\n>b\nACJT\n",
            Alphabet::Dna,
            ValidationMode::Permissive,
            None,
        )
        .unwrap();
        assert_eq!(&p.rows[4..], b"AC\0T");
        assert_eq!(p.sequence_stats.unknown.count, 1);
        assert_eq!(
            p.sequence_stats.unknown.examples,
            ["'J' (0x4A) at record 2, line 4, column 3"]
        );
        // Non-ASCII bytes are unknown too.
        let p = run(
            ">a\nAC\u{e9}T\n>b\nACGTT\n".as_bytes(),
            Alphabet::Dna,
            ValidationMode::Report,
            None,
        )
        .unwrap();
        assert_eq!(p.sequence_stats.unknown.count, 2, "both bytes of é");
    }

    #[test]
    fn structural_errors() {
        let e = run(
            b">a\nACGT\n>b\nAC\0T\n",
            Alphabet::Dna,
            ValidationMode::Permissive,
            None,
        )
        .unwrap_err();
        assert!(e.to_string().contains("NUL byte"));
        assert_eq!(e.class(), crate::error::ExitClass::InvalidInput);
        let e = run(
            b">a\nACGT\n>b\nACG\n",
            Alphabet::Dna,
            ValidationMode::Strict,
            None,
        )
        .unwrap_err();
        assert_eq!(
            e.to_string(),
            "t.fa: record 2 (line 3): the sequence has 3 alignment columns, but the first record has 4"
        );
        assert!(e.hint().unwrap().contains("MAFFT"));
        let e = run(
            b">a\nACGT\n>b\n\n",
            Alphabet::Dna,
            ValidationMode::Strict,
            None,
        )
        .unwrap_err();
        assert!(
            e.to_string()
                .contains("record 2 (line 3): the sequence is empty")
        );
        let e = run(
            b">a\nACGT\n>b\nACGTA\n",
            Alphabet::Dna,
            ValidationMode::Strict,
            None,
        )
        .unwrap_err();
        assert!(e.to_string().contains("5 alignment columns"));
    }

    #[test]
    fn headers_in_order_across_chunks() {
        let mut text = Vec::new();
        for i in 0..200 {
            text.extend_from_slice(
                format!(">ACC{i}|{}\nACGT\n", ["Camel", "Human"][i % 2]).as_bytes(),
            );
        }
        let spec = HeaderSpec::new(
            &["Accession".into(), "Host".into()],
            Some(&["Host".into()]),
            "U",
        )
        .unwrap();
        let out = run(&text, Alphabet::Dna, ValidationMode::Strict, Some(&spec)).unwrap();
        assert_eq!(out.records, 200);
        assert_eq!(out.header_values.len(), 200);
        assert_eq!(out.header_values[0], ["Camel"]);
        assert_eq!(out.header_values[199], ["Human"]);
        assert_eq!(out.header_stats.records, 200);
        // Strict mode reports the first error in input order even across chunks.
        let mut bad = text.clone();
        bad.extend_from_slice(b">X|Y\nAC\xffT\n>Z|W\nACG\n");
        let e = run(&bad, Alphabet::Dna, ValidationMode::Strict, Some(&spec)).unwrap_err();
        assert!(e.to_string().contains("record 201"), "{e}");
    }

    #[test]
    fn describe_bytes() {
        assert_eq!(describe_byte(b'J'), "'J' (0x4A)");
        assert_eq!(describe_byte(0xC3), "0xC3");
        assert_eq!(describe_byte(b' '), "0x20");
    }
}
