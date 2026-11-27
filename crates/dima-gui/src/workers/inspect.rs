//! Opening a file: decide by content whether it is a results file or an alignment (G64), then
//! read the results or run the library's first-batch pre-check (D85). A file compressed in a
//! way neither is read from is named for what it is, and a bzip2 file, an alignment as far as
//! the library is concerned, is told apart from results compressed with bzip2, which DiMA does
//! not read.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::AtomicBool;

use dima_core::error::{IoAction, IoTarget};
use dima_core::input::Source;
use dima_core::model::Results;
use dima_core::output::{RESULTS_SIGNATURE_LEN, unreadable_results_compression};
use dima_core::validation::{self, Inspection};
use dima_core::{DimaError, Result};

/// What a file turned out to be.
#[derive(Debug)]
pub enum Opened {
    /// An alignment, with the pre-check summary.
    Alignment(Box<Inspection>),
    /// A results file written by DiMA (CLI or app).
    Results(Box<Results>),
}

/// The content kinds told apart by [`sniff`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A JSON object (results files are plain, gzip or Zstandard JSON).
    Json,
    /// Compressed with bzip2: an alignment (the library reads them), or results, which it
    /// does not read; only decompressing tells, which the pre-check does.
    Bzip2,
    /// Compressed or archived in a format DiMA reads neither alignments nor results from,
    /// named in words (`xz (.xz)`, `a zip archive`).
    Unreadable(&'static str),
    /// Anything else: treated as an alignment (the pre-check reports what is wrong with it).
    Other,
}

/// Looks at the first decoded bytes: a JSON object (after an optional byte-order mark and white
/// space) is a results file. Only gzip and Zstandard are decoded here, the compressions results
/// files can have; bzip2 and the formats DiMA does not read are recognised by their signature
/// (the library's), and every other input is left to the alignment reader.
///
/// # Errors
/// [`DimaError::Io`] when the file cannot be opened or read.
pub fn sniff(path: &Path) -> Result<Kind> {
    let target = || IoTarget::Path(path.to_path_buf());
    let mut file = File::open(path).map_err(|e| DimaError::io(IoAction::Open, target(), e))?;
    let mut magic = [0u8; RESULTS_SIGNATURE_LEN];
    let n = read_up_to(&mut file, &mut magic)
        .map_err(|e| DimaError::io(IoAction::Read, target(), e))?;
    let magic = &magic[..n];
    if let Some(what) = unreadable_results_compression(magic) {
        return Ok(if is_bzip2(magic) {
            Kind::Bzip2
        } else {
            Kind::Unreadable(what)
        });
    }
    let reopened = io::Cursor::new(magic.to_vec()).chain(file);
    let mut decoded: Box<dyn Read> = match magic {
        [0x1F, 0x8B, ..] => Box::new(flate2::read::MultiGzDecoder::new(reopened)),
        // A Zstandard frame, or a skippable frame first (as pzstd writes), as the library reads.
        [0x28, 0xB5, 0x2F, 0xFD, ..] | [0x50..=0x5F, 0x2A, 0x4D, 0x18, ..] => {
            match zstd::stream::read::Decoder::new(reopened) {
                Ok(d) => Box::new(d),
                Err(_) => return Ok(Kind::Other),
            }
        }
        _ => Box::new(reopened),
    };
    let mut head = [0u8; 256];
    // A decoding error is not this function's to report: the reader that follows reports it.
    let Ok(n) = read_up_to(&mut decoded, &mut head) else {
        return Ok(Kind::Other);
    };
    let text = head[..n]
        .strip_prefix(b"\xEF\xBB\xBF")
        .unwrap_or(&head[..n]);
    let first = text.iter().find(|b| !b.is_ascii_whitespace());
    Ok(if first == Some(&b'{') {
        Kind::Json
    } else {
        Kind::Other
    })
}

/// bzip2's signature: "BZh" and the block size, as the library's reader recognises it.
const fn is_bzip2(prefix: &[u8]) -> bool {
    matches!(prefix, [b'B', b'Z', b'h', b'1'..=b'9', ..])
}

fn read_up_to(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

/// Opens `path` as a results file or an alignment, by content. Reading results stops soon after
/// `cancel` is set (the pre-check reads only the first batch).
///
/// # Errors
/// The errors of [`sniff`], of [`load_results`] or of the pre-check; for a file compressed in a
/// way DiMA reads nothing from, or results compressed with bzip2, an error that says so.
pub fn open(path: &Path, cancel: &AtomicBool) -> Result<Opened> {
    let inspect = || {
        validation::inspect(&Source::Path(path.to_path_buf()))
            .map(|i| Opened::Alignment(Box::new(i)))
    };
    match sniff(path)? {
        Kind::Json => load_results(path, cancel).map(Opened::Results),
        Kind::Unreadable(what) => Err(unreadable(path, what)),
        // The pre-check decompresses it: text that starts as a JSON object is results.
        Kind::Bzip2 => match inspect() {
            Err(e) if starts_as_json(&e) => Err(bzip2_results(path)),
            other => other,
        },
        Kind::Other => inspect(),
    }
}

/// `true` for the pre-check's error on text whose first character is `{` where a FASTA header
/// line was expected, the start of every results file.
fn starts_as_json(e: &DimaError) -> bool {
    let found = format!("found {}", dima_core::normalise::describe_byte(b'{'));
    matches!(
        e,
        DimaError::InputFormat { location, message, .. }
            if location.line == Some(1) && message.ends_with(&found)
    )
}

/// The error for a file compressed or archived in a format DiMA reads neither alignments nor
/// results from.
fn unreadable(path: &Path, what: &str) -> DimaError {
    let name = dima_core::run_report::file_name(path);
    // The library's words are a format ("xz (.xz)") or an archive ("a zip archive").
    let (is, hint) = if what.starts_with("a ") {
        (
            format!("is {what}"),
            "extract the alignment or the results file from the archive, then open that file",
        )
    } else {
        (
            format!("is compressed with {what}"),
            "decompress it, or recompress it with gzip or Zstandard, and open it again",
        )
    };
    DimaError::input_data(None, format!("'{name}' {is}, which DiMA does not read")).with_hint(hint)
}

/// The error for results compressed with bzip2, which DiMA reads alignments from but not
/// results.
fn bzip2_results(path: &Path) -> DimaError {
    let name = dima_core::run_report::file_name(path);
    DimaError::input_data(
        None,
        format!("'{name}' is compressed with bzip2, which DiMA does not read results from"),
    )
    .with_hint(
        "decompress it, or recompress it with gzip or Zstandard, and open it again \
         (alignments compressed with bzip2 open as they are)",
    )
}

/// Reads a results file and checks that its positions are distinct, sorted on load (G25):
/// every view indexes positions in order.
///
/// # Errors
/// The errors of `read_results` (which include repeated positions), and
/// [`DimaError::Cancelled`].
pub fn load_results(path: &Path, cancel: &AtomicBool) -> Result<Box<Results>> {
    // The library sorts positions listed out of order (G25) and rejects repeated ones.
    let r = dima_core::output::read_results_cancellable(path, cancel)?;
    dima_core::output::check_finite(&r)?;
    Ok(Box::new(r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sniffs_results_and_alignments_by_content() {
        let dir = tempfile::tempdir().unwrap();
        let json = dir.path().join("results.fasta"); // misleading extension on purpose
        std::fs::write(&json, "\u{FEFF}  \n{\"schema_version\":\"1.0.0\"}").unwrap();
        assert_eq!(sniff(&json).unwrap(), Kind::Json);
        let gz = dir.path().join("r.gz");
        let mut e =
            flate2::write::GzEncoder::new(File::create(&gz).unwrap(), flate2::Compression::fast());
        e.write_all(b"{\"a\":1}").unwrap();
        e.finish().unwrap();
        assert_eq!(sniff(&gz).unwrap(), Kind::Json);
        let zst = dir.path().join("r.zst");
        std::fs::write(&zst, zstd::encode_all(&b"\n{}"[..], 3).unwrap()).unwrap();
        assert_eq!(sniff(&zst).unwrap(), Kind::Json);
        let fasta = dir.path().join("a.json"); // misleading extension on purpose
        std::fs::write(&fasta, ">s1\nACGT\n").unwrap();
        assert_eq!(sniff(&fasta).unwrap(), Kind::Other);
        let empty = dir.path().join("empty");
        std::fs::write(&empty, "").unwrap();
        assert_eq!(sniff(&empty).unwrap(), Kind::Other);
        let missing = dir.path().join("missing");
        let e = sniff(&missing).unwrap_err();
        assert_eq!(e.class(), dima_core::error::ExitClass::Io);
        assert!(
            e.to_string().contains("missing"),
            "errors carry the path (G34)"
        );
    }

    /// `{"schema_version":"1.0.0"}` and a line feed, compressed by `bzip2 -9`.
    const BZIP2_JSON: [u8; 70] = [
        0x42, 0x5a, 0x68, 0x39, 0x31, 0x41, 0x59, 0x26, 0x53, 0x59, 0x05, 0x2b, 0xb1, 0x19, 0x00,
        0x00, 0x0c, 0xdb, 0x80, 0x00, 0x10, 0x10, 0x01, 0x60, 0x10, 0x00, 0x00, 0xaa, 0x63, 0x99,
        0x0a, 0x20, 0x00, 0x21, 0xa8, 0x68, 0x34, 0x06, 0x8d, 0xa8, 0x53, 0x09, 0xa6, 0x80, 0xd3,
        0x10, 0xd7, 0x86, 0xc1, 0x54, 0x28, 0xc3, 0x3b, 0x16, 0x4e, 0xbb, 0xca, 0xe4, 0x94, 0xf8,
        0xbb, 0x92, 0x29, 0xc2, 0x84, 0x80, 0x29, 0x5d, 0x88, 0xc8,
    ];

    /// `hello` and a line feed, compressed by `bzip2 -9`.
    const BZIP2_HELLO: [u8; 42] = [
        0x42, 0x5a, 0x68, 0x39, 0x31, 0x41, 0x59, 0x26, 0x53, 0x59, 0xc1, 0xc0, 0x80, 0xe2, 0x00,
        0x00, 0x01, 0x41, 0x00, 0x00, 0x10, 0x02, 0x44, 0xa0, 0x00, 0x30, 0xcd, 0x00, 0xc3, 0x46,
        0x29, 0x97, 0x17, 0x72, 0x45, 0x38, 0x50, 0x90, 0xc1, 0xc0, 0x80, 0xe2,
    ];

    /// Results compressed with bzip2 are named so, instead of being read as a broken
    /// alignment; bzip2 alignments still open; xz and zip files are named for what they are,
    /// whatever their name.
    #[test]
    fn compressed_results_are_told_apart_from_alignments() {
        let dir = tempfile::tempdir().unwrap();
        let results = dir.path().join("mers.json.bz2");
        std::fs::write(&results, BZIP2_JSON).unwrap();
        assert_eq!(sniff(&results).unwrap(), Kind::Bzip2);
        let e = open(&results, &AtomicBool::new(false)).unwrap_err();
        assert_eq!(
            e.to_string(),
            "'mers.json.bz2' is compressed with bzip2, which DiMA does not read results from"
        );
        assert!(
            e.hint()
                .unwrap()
                .contains("recompress it with gzip or Zstandard")
        );
        // A bzip2 alignment is an alignment.
        let nt = crate::tests::fixtures::mers_nt_path();
        assert_eq!(sniff(&nt).unwrap(), Kind::Bzip2);
        assert!(matches!(
            open(&nt, &AtomicBool::new(false)).unwrap(),
            Opened::Alignment(_)
        ));
        // A bzip2 file that is neither keeps the pre-check's own error.
        let text = dir.path().join("notes.bz2");
        std::fs::write(&text, BZIP2_HELLO).unwrap();
        let e = open(&text, &AtomicBool::new(false)).unwrap_err();
        assert!(!e.to_string().contains("results from"), "{e}");
        assert!(e.to_string().contains("found 'h'"), "{e}");
        // Formats DiMA reads nothing from, named by content.
        let xz = dir.path().join("results.json");
        std::fs::write(&xz, [0xFD, b'7', b'z', b'X', b'Z', 0x00, 0x00, 0x04]).unwrap();
        assert_eq!(sniff(&xz).unwrap(), Kind::Unreadable("xz (.xz)"));
        let e = open(&xz, &AtomicBool::new(false)).unwrap_err();
        assert_eq!(
            e.to_string(),
            "'results.json' is compressed with xz (.xz), which DiMA does not read"
        );
        assert_eq!(
            e.hint(),
            Some("decompress it, or recompress it with gzip or Zstandard, and open it again")
        );
        let zip = dir.path().join("download.zip");
        std::fs::write(&zip, b"PK\x03\x04\x14\x00\x00\x00").unwrap();
        let e = open(&zip, &AtomicBool::new(false)).unwrap_err();
        assert_eq!(
            e.to_string(),
            "'download.zip' is a zip archive, which DiMA does not read"
        );
        assert!(
            e.hint()
                .unwrap()
                .starts_with("extract the alignment or the results file")
        );
    }

    /// A Zstandard file that starts with a skippable frame (as pzstd writes it) is read like
    /// any other Zstandard file, as the library reads it.
    #[test]
    fn a_skippable_zstandard_frame_is_zstandard() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("r.zst");
        let mut bytes = vec![0x50, 0x2A, 0x4D, 0x18, 4, 0, 0, 0, 1, 2, 3, 4];
        bytes.extend(zstd::encode_all(&b"{\"a\":1}"[..], 3).unwrap());
        std::fs::write(&p, bytes).unwrap();
        assert_eq!(sniff(&p).unwrap(), Kind::Json);
    }

    #[test]
    fn opens_the_sample_alignment() {
        let p = crate::tests::fixtures::mers_path();
        match open(&p, &AtomicBool::new(false)).unwrap() {
            Opened::Alignment(i) => {
                assert_eq!(i.records_examined, 191);
                assert_eq!(i.alignment_length, 1353);
            }
            Opened::Results(_) => panic!("an alignment"),
        }
    }

    #[test]
    fn results_round_trip_and_repeated_positions_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let r = crate::tests::fixtures::mers();
        let p = dir.path().join("out.json.zst");
        let mut sink = dima_core::output::Sink::create(
            &dima_core::output::Target::Path(p.clone()),
            dima_core::output::OutputCompression::Zstd,
        )
        .unwrap();
        dima_core::output::write_json(&r, false, &mut sink).unwrap();
        sink.commit().unwrap();
        match open(&p, &AtomicBool::new(false)).unwrap() {
            Opened::Results(back) => assert_eq!(*back, *r),
            Opened::Alignment(_) => panic!("a results file"),
        }
        // Shuffled positions are sorted; a repeated position is an error.
        let mut shuffled = (*r).clone();
        shuffled.results.swap(0, 5);
        let q = dir.path().join("shuffled.json");
        std::fs::write(&q, serde_json::to_string(&shuffled).unwrap()).unwrap();
        let back = load_results(&q, &AtomicBool::new(false)).unwrap();
        assert!(
            back.results
                .windows(2)
                .all(|w| w[0].position < w[1].position)
        );
        let mut dup = (*r).clone();
        dup.results[1].position = dup.results[0].position;
        let q = dir.path().join("dup.json");
        std::fs::write(&q, serde_json::to_string(&dup).unwrap()).unwrap();
        assert!(
            load_results(&q, &AtomicBool::new(false))
                .unwrap_err()
                .to_string()
                .contains("more than once")
        );
    }

    #[test]
    fn a_cancelled_load_stops() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("r.json");
        std::fs::write(
            &p,
            serde_json::to_vec(&*crate::tests::fixtures::mers()).unwrap(),
        )
        .unwrap();
        let e = open(&p, &AtomicBool::new(true)).unwrap_err();
        assert!(matches!(e, DimaError::Cancelled), "{e}");
    }
}
