//! Opening a file: decide by content whether it is a results file or an alignment (G64), then
//! read the results or run the library's first-batch pre-check (D85).

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::AtomicBool;

use dima_core::error::{IoAction, IoTarget};
use dima_core::input::Source;
use dima_core::model::Results;
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
    /// Anything else: treated as an alignment (the pre-check reports what is wrong with it).
    Other,
}

/// Looks at the first decoded bytes: a JSON object (after an optional byte-order mark and white
/// space) is a results file. Only gzip and Zstandard are decoded here, the compressions results
/// files can have; every other input is left to the alignment reader.
///
/// # Errors
/// [`DimaError::Io`] when the file cannot be opened or read.
pub fn sniff(path: &Path) -> Result<Kind> {
    let target = || IoTarget::Path(path.to_path_buf());
    let mut file = File::open(path).map_err(|e| DimaError::io(IoAction::Open, target(), e))?;
    let mut magic = [0u8; 4];
    let n = read_up_to(&mut file, &mut magic)
        .map_err(|e| DimaError::io(IoAction::Read, target(), e))?;
    let reopened = io::Cursor::new(magic[..n].to_vec()).chain(file);
    let mut decoded: Box<dyn Read> = match &magic[..n] {
        [0x1F, 0x8B, ..] => Box::new(flate2::read::MultiGzDecoder::new(reopened)),
        [0x28, 0xB5, 0x2F, 0xFD] => match zstd::stream::read::Decoder::new(reopened) {
            Ok(d) => Box::new(d),
            Err(_) => return Ok(Kind::Other),
        },
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
/// The errors of [`sniff`], of [`load_results`] or of the pre-check.
pub fn open(path: &Path, cancel: &AtomicBool) -> Result<Opened> {
    match sniff(path)? {
        Kind::Json => load_results(path, cancel).map(Opened::Results),
        Kind::Other => validation::inspect(&Source::Path(path.to_path_buf()))
            .map(|i| Opened::Alignment(Box::new(i))),
    }
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
