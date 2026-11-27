//! Opening an input and reading it as batches of raw FASTA records.
//!
//! Pipeline: the stored bytes are read once (files, FIFOs, `<(…)` and standard input alike) and
//! teed to a SHA-256 thread; the format is recognised from the first bytes (gzip/BGZF, bzip2,
//! Zstandard or plain text; unsupported formats, xz among them (D116), get an explicit error with
//! a hint); the decompressed
//! text has one UTF-8 byte-order mark and the blank lines before the first record removed
//! (UTF-16/32 is refused with an `iconv` hint); and `seq_io` splits it into records, which are
//! copied byte for byte into batches for the parallel normaliser. Every structural error stops
//! the run with the source and as much of the record, line and byte as applies.

use std::fs::File;
use std::io::{self, BufReader, Cursor, Read};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::thread;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use crate::alphabet;
use crate::config::{BATCH_MAX_BYTES, BATCH_MAX_RECORDS, PROBE_MIN_RECORDS};
use crate::control::{Hook, Progress};
use crate::error::{DimaError, IoAction, IoTarget, Location, Result};
use crate::model::{Checksum, Compression, InputInfo};

mod members;

use members::{Members, TrailingData};

/// Size of the buffer between the stored bytes and the decoder.
const RAW_BUFFER: usize = 1 << 20;
/// Chunks in flight to the checksum thread.
const HASH_QUEUE: usize = 16;

/// Where an input comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A file system path (regular file, FIFO, `/dev/stdin`, process substitution, …).
    Path(PathBuf),
    /// Standard input.
    Stdin,
}

/// `true` for a path that names an open descriptor rather than a file: `/dev/stdin`,
/// `/dev/stdout`, `/dev/stderr`, `/dev/fd/N` (process substitution, `<(...)`) and
/// `/proc/<pid>/fd/N`. Its file name is a descriptor number chosen by the shell, so it names
/// nothing (D45: such inputs are named like standard input).
#[must_use]
pub fn is_descriptor_alias(p: &Path) -> bool {
    if !cfg!(unix) {
        return false;
    }
    let components: Vec<_> = p.components().map(|c| c.as_os_str().to_owned()).collect();
    let is = |i: usize, s: &str| components.get(i).is_some_and(|c| c == s);
    (is(1, "dev") && (is(2, "stdout") || is(2, "stderr") || is(2, "stdin") || is(2, "fd")))
        || (is(1, "proc") && is(3, "fd"))
}

impl Source {
    /// Name used in messages: the path as given, or `standard input`.
    #[must_use]
    pub fn display_name(&self) -> String {
        match self {
            Self::Path(p) => p.display().to_string(),
            Self::Stdin => "standard input".to_owned(),
        }
    }

    /// Name recorded in the provenance: the file name without directories, or `-` for
    /// standard input and descriptor paths ([`is_descriptor_alias`]).
    #[must_use]
    pub fn provenance_name(&self) -> String {
        match self {
            Self::Path(p) if is_descriptor_alias(p) => "-".to_owned(),
            Self::Path(p) => p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            Self::Stdin => "-".to_owned(),
        }
    }

    fn target(&self) -> IoTarget {
        match self {
            Self::Path(p) => IoTarget::Path(p.clone()),
            Self::Stdin => IoTarget::Stdin,
        }
    }
}

/// Extensions of the compressions DiMA reads (file dialogs offer them).
pub const COMPRESSION_EXTENSIONS: &[&str] = &[".gz", ".bgz", ".bz2", ".zst"];
/// Extensions of FASTA alignments.
pub const FASTA_EXTENSIONS: &[&str] = &[
    ".fasta", ".fas", ".fa", ".fna", ".ffn", ".faa", ".frn", ".afa", ".aln", ".mfa",
];
/// Also removed from query names, although xz is not read (D116): `spike.fasta.xz.zst` is still
/// `spike`.
const UNREAD_COMPRESSION_EXTENSIONS: &[&str] = &[".xz"];

/// The query name implied by an input's file name (D45): the file name without known
/// compression and FASTA extensions, or `None` for standard input or an empty stem.
#[must_use]
pub fn query_name_from_source(source: &Source) -> Option<String> {
    let Source::Path(p) = source else {
        return None;
    };
    if is_descriptor_alias(p) {
        return None;
    }
    let mut name = p.file_name()?.to_string_lossy().into_owned();
    let strip = |name: &mut String, list: &[&str]| {
        let lower = name.to_ascii_lowercase();
        if let Some(ext) = list.iter().find(|e| lower.ends_with(*e)) {
            name.truncate(name.len() - ext.len());
            true
        } else {
            false
        }
    };
    // Removed repeatedly and case-insensitively, after MultiQC's sample-name cleaning (Ewels et
    // al. 2016).
    while strip(&mut name, COMPRESSION_EXTENSIONS)
        || strip(&mut name, UNREAD_COMPRESSION_EXTENSIONS)
    {}
    while strip(&mut name, FASTA_EXTENSIONS) {}
    let name = name.trim().to_owned();
    (!name.is_empty()).then_some(name)
}

/// The stored bytes, shared between the decoding chain and the final drain.
#[derive(Clone)]
enum RawHandle {
    File(Arc<File>),
    Stdin,
}

impl Read for RawHandle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::File(f) => (&**f).read(buf),
            Self::Stdin => io::stdin().read(buf),
        }
    }
}

/// Marks an error raised while reading the stored bytes (an I/O error, exit 3), as opposed to
/// one raised while decoding them (corrupt data, exit 4).
#[derive(Debug)]
pub(crate) struct StoredReadError(pub(crate) io::Error);

impl std::fmt::Display for StoredReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for StoredReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Reads the stored bytes, forwarding a copy of every chunk to the checksum thread and counting
/// them for progress.
struct Tee {
    raw: RawHandle,
    tx: SyncSender<Vec<u8>>,
    progress: Arc<Progress>,
}

impl Tee {
    fn duplicate(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            tx: self.tx.clone(),
            progress: Arc::clone(&self.progress),
        }
    }
}

impl Read for Tee {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.raw.read(buf) {
                Ok(n) => {
                    if n > 0 {
                        // A closed receiver only means the checksum is no longer wanted.
                        let _ = self.tx.send(buf[..n].to_vec());
                        self.progress.add_bytes_read(n as u64);
                    }
                    return Ok(n);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(io::Error::new(e.kind(), StoredReadError(e))),
            }
        }
    }
}

/// The SHA-256 of the stored bytes, computed on its own thread.
pub struct ChecksumJob {
    /// Always `Some` until [`ChecksumJob::finish`].
    handle: Option<thread::JoinHandle<([u8; 32], u64)>>,
}

impl ChecksumJob {
    fn spawn(thread_start: Option<Hook>) -> Result<(Self, SyncSender<Vec<u8>>)> {
        let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(HASH_QUEUE);
        let handle = thread::Builder::new()
            .name("dima-sha256".into())
            .spawn(move || {
                if let Some(h) = &thread_start {
                    h.call();
                }
                let mut hasher = Sha256::new();
                let mut size = 0u64;
                for chunk in rx {
                    hasher.update(&chunk);
                    size += chunk.len() as u64;
                }
                (hasher.finalize().into(), size)
            })
            .map_err(|e| DimaError::system("the checksum thread", e))?;
        Ok((
            Self {
                handle: Some(handle),
            },
            tx,
        ))
    }

    /// Waits for the digest (every sender must have been dropped) and returns it with the
    /// number of stored bytes.
    ///
    /// # Errors
    /// [`DimaError::Internal`] if the checksum thread panicked.
    pub fn finish(mut self) -> Result<(Checksum, u64)> {
        let (digest, size) = self
            .handle
            .take()
            .ok_or_else(|| DimaError::internal("the checksum was finished twice"))?
            .join()
            .map_err(|_| DimaError::internal("the checksum thread panicked"))?;
        let value = digest.iter().fold(String::with_capacity(64), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        });
        Ok((
            Checksum {
                kind: "sha-256".to_owned(),
                value,
            },
            size,
        ))
    }
}

/// Reads until `buf` is full or the stream ends; returns the number of bytes read.
fn read_up_to(r: &mut impl Read, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

/// What the first bytes of the stored data say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Sniffed {
    Supported(Compression),
    Unsupported(&'static str, &'static str),
}

pub(crate) fn sniff(prefix: &[u8]) -> Sniffed {
    use Compression as C;
    match prefix {
        [0x1F, 0x8B, ..] => Sniffed::Supported(C::Gzip),
        [b'B', b'Z', b'h', b'1'..=b'9', ..] => Sniffed::Supported(C::Bzip2),
        [0xFD, b'7', b'z', b'X', b'Z', 0x00, ..] => Sniffed::Unsupported(
            "xz (.xz)",
            "decompress it first (`xz -dk NAME.xz`, or 7-Zip on Windows) or recompress it with \
             Zstandard (`xz -dc NAME.xz | zstd -19 -o NAME.zst`)",
        ),
        [0x28, 0xB5, 0x2F, 0xFD, ..] => Sniffed::Supported(C::Zstd),
        // A Zstandard skippable frame (0x184D2A50–5F), as written first by pzstd.
        [b, 0x2A, 0x4D, 0x18, ..] if b & 0xF0 == 0x50 => Sniffed::Supported(C::Zstd),
        [0x1F, 0x9D, ..] => Sniffed::Unsupported(
            "Unix compress (.Z)",
            "decompress it first (`gzip -d` or `uncompress`) or recompress with gzip or zstd",
        ),
        [0x04, 0x22, 0x4D, 0x18, ..] => Sniffed::Unsupported(
            "LZ4",
            "decompress it first (`lz4 -d`) or recompress with gzip or zstd",
        ),
        [0x5D, 0x00, 0x00, ..] => Sniffed::Unsupported(
            "legacy LZMA (.lzma)",
            "decompress it first (`unlzma`) or recompress with gzip or zstd",
        ),
        [b'P', b'K', 0x03, 0x04, ..] => Sniffed::Unsupported(
            "a zip archive",
            "extract the FASTA file from the archive first",
        ),
        [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C, ..] => Sniffed::Unsupported(
            "a 7z archive",
            "extract the FASTA file from the archive first",
        ),
        _ => Sniffed::Supported(C::None),
    }
}

/// Strips one UTF-8 byte-order mark and refuses UTF-16/32 text.
fn handle_bom(
    mut stream: Box<dyn Read + Send>,
    source: &Source,
    compression: Compression,
) -> Result<(Box<dyn Read + Send>, u64)> {
    let mut head = [0u8; 4];
    // The first block is decoded here, so a damaged file can fail already.
    let n = read_up_to(&mut stream, &mut head)
        .map_err(|e| map_stream_error(e, source, Some(compression)))?;
    let head = &head[..n];
    // UTF-32 first: its little-endian mark starts like UTF-16's.
    let encoding = match head {
        [0xFF, 0xFE, 0x00, 0x00] | [0x00, 0x00, 0xFE, 0xFF] => Some("UTF-32"),
        [0xFF, 0xFE, ..] | [0xFE, 0xFF, ..] => Some("UTF-16"),
        _ => None,
    };
    if let Some(encoding) = encoding {
        return Err(DimaError::input_format(
            Location::new(source.display_name()).byte(0),
            format!("the text is {encoding} encoded"),
        )
        .with_hint(format!(
            "convert it to UTF-8 or ASCII, e.g. `iconv -f {encoding} -t UTF-8 in.fasta > out.fasta`"
        )));
    }
    let skip = if head.starts_with(&[0xEF, 0xBB, 0xBF]) {
        3
    } else {
        0
    };
    let rest = head[skip..].to_vec();
    Ok((Box::new(Cursor::new(rest).chain(stream)), skip as u64))
}

/// Where the text the FASTA parser reads starts in the decoded input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TextStart {
    /// Bytes before it (the byte-order mark and the lines skipped).
    bytes: u64,
    /// Lines skipped before it.
    lines: u64,
}

/// Skips the lines before the first record that are empty or hold only white space (space,
/// TAB, CR, VT, FF: the bytes ignored inside sequence lines, D37), as MAFFT, Clustal Omega,
/// EMBOSS and HMMER's Easel do. The parser would refuse a white-space line and miscounts lines
/// and bytes after more than one buffer of blank lines (seq_io 0.3.4 `fasta.rs` `init`), so
/// the lines are skipped and counted here and every location is offset instead. Only the
/// current line's white space is counted, never kept, so memory stays bounded however long the
/// lines are. A line whose first other byte follows white space (an indented `>`, a stray or
/// classic Mac OS carriage return) is refused here with its location.
///
/// Text with classic Mac OS line endings (CR only) is refused here too, from the text already
/// read (up to 64 KiB; never waiting for more, so a pipe that is still being written never
/// blocks the caller): the parser would otherwise hold the whole file as one line before finding
/// the CR.
fn skip_blank_lines(
    mut stream: Box<dyn Read + Send>,
    source: &Source,
    compression: Compression,
    bom: u64,
) -> Result<(Box<dyn Read + Send>, TextStart)> {
    let mut buf: Vec<u8> = Vec::new();
    let mut i = 0;
    let mut start = TextStart {
        bytes: bom,
        lines: 0,
    };
    // The current line so far: its white-space bytes (examined and dropped) and the offset of
    // its first carriage return.
    let mut column: u64 = 0;
    let mut first_cr: Option<u64> = None;
    let mut ended = false;
    loop {
        if i == buf.len() {
            if ended {
                break;
            }
            buf.clear();
            i = 0;
            buf.resize(CR_ONLY_PEEK, 0);
            let n = loop {
                match stream.read(&mut buf) {
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    r => break r.map_err(|e| map_stream_error(e, source, Some(compression)))?,
                }
            };
            buf.truncate(n);
            ended = n == 0;
            continue;
        }
        let b = buf[i];
        if b == b'\n' {
            start.bytes += column + 1;
            start.lines += 1;
            column = 0;
            first_cr = None;
        } else if alphabet::is_whitespace(b) {
            if b == b'\r' && first_cr.is_none() {
                first_cr = Some(column);
            }
            column += 1;
        } else if column == 0 {
            break;
        } else {
            return Err(indented_line(
                source,
                start,
                column,
                first_cr,
                b,
                buf.get(i + 1),
            ));
        }
        i += 1;
    }
    let mut rest = buf.split_off(i);
    // The first read can be only the few bytes the format check looked at: without a line feed
    // so far, read once more (what is available, not until 64 KiB).
    if !ended && rest.len() < CR_ONLY_PEEK && memchr::memchr(b'\n', &rest).is_none() {
        let before = rest.len();
        rest.resize(CR_ONLY_PEEK, 0);
        let n = loop {
            match stream.read(&mut rest[before..]) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                r => break r.map_err(|e| map_stream_error(e, source, Some(compression)))?,
            }
        };
        rest.truncate(before + n);
    }
    if let (None, Some(cr)) = (memchr::memchr(b'\n', &rest), memchr::memchr(b'\r', &rest)) {
        return Err(DimaError::input_format(
            Location::new(source.display_name())
                .record(1)
                .line(start.lines + 1)
                .byte(start.bytes + cr as u64),
            "carriage return inside a header line",
        )
        .with_hint(CR_ONLY_HINT));
    }
    Ok((Box::new(Cursor::new(rest).chain(stream)), start))
}

/// Hint for text with classic Mac OS line endings.
const CR_ONLY_HINT: &str = "the file uses classic Mac OS line endings (CR only); convert them, \
                            e.g. `tr '\\r' '\\n' < in.fasta > out.fasta`";

/// The error for a line before the first record that has white space and then `found` (with
/// `next` after it, when read): `column` white-space bytes, the first carriage return at
/// `first_cr`.
fn indented_line(
    source: &Source,
    start: TextStart,
    column: u64,
    first_cr: Option<u64>,
    found: u8,
    next: Option<&u8>,
) -> DimaError {
    let at = |offset: u64| {
        Location::new(source.display_name())
            .line(start.lines + 1)
            .column(offset + 1)
            .byte(start.bytes + offset)
    };
    if let Some(cr) = first_cr {
        // A carriage return that does not end a line: classic Mac OS line endings, or a stray
        // one; turning it into a line feed fixes both.
        return DimaError::input_format(at(cr), "carriage return before the first header line")
            .with_hint(
                "the file uses classic Mac OS line endings (CR only) or has a stray carriage \
                 return; convert carriage returns to line feeds, e.g. \
                 `tr '\\r' '\\n' < in.fasta > out.fasta`",
            );
    }
    if found == b'>' {
        return DimaError::input_format(at(column), "white space before the first header's '>'")
            .with_hint("a header line starts with '>'; remove the white space before it");
    }
    DimaError::input_format(
        at(column),
        format!(
            "expected a '>' header line, found {}",
            crate::normalise::describe_byte(found)
        ),
    )
    .with_hint(start_hint(found, next.copied()))
}

/// Hint for a first line that does not start with `>`: its first byte `found`, then `next`.
fn start_hint(found: u8, next: Option<u8>) -> &'static str {
    match (found, next) {
        (0, _) => {
            "the file is binary, not FASTA text (or UTF-16/UTF-32 text without a byte-order \
             mark; convert it with `iconv`)"
        }
        (0xC2, Some(0xA0) | None) => {
            "0xC2 0xA0 is a no-break space (U+00A0), which is not white space in FASTA; delete \
             it or replace it with a space"
        }
        _ => {
            "DiMA reads aligned FASTA; if this is an archive (tar) or another format, extract \
             or convert it first"
        }
    }
}

/// How much of the text is examined for classic Mac OS line endings.
const CR_ONLY_PEEK: usize = 1 << 16;

/// Turns an error raised while reading the text into a [`DimaError`]: stored-byte read errors
/// are I/O errors; everything else is corrupt or truncated compressed data.
fn map_stream_error(e: io::Error, source: &Source, compression: Option<Compression>) -> DimaError {
    let kind = e.kind();
    match e.into_inner() {
        Some(inner) => match inner.downcast::<StoredReadError>() {
            // `dima analyze < folder`: the same mistake as `-i folder`.
            Ok(stored) if stored.0.kind() == io::ErrorKind::IsADirectory => {
                DimaError::usage(format!("{} is a directory", source.display_name()))
                    .with_hint("give the alignment file with -i, or redirect a file")
            }
            Ok(stored) => DimaError::io(IoAction::Read, source.target(), stored.0),
            Err(other) => match other.downcast::<TrailingData>() {
                Ok(t) => trailing(source, &t),
                Err(other) => corrupt(source, compression, &other.to_string(), kind),
            },
        },
        None => corrupt(source, compression, &kind.to_string(), kind),
    }
}

/// Data after the end of the compressed data (not padding): `gzip -d` and `bzip2 -d` ignore it
/// with a warning, so their test commands would call the file fine.
fn trailing(source: &Source, t: &TrailingData) -> DimaError {
    let (tool, ext) = if t.format == "bzip2" {
        ("bzip2", "bz2")
    } else {
        ("gzip", "gz")
    };
    DimaError::input_format(Location::new(source.display_name()), t.to_string()).with_hint(format!(
        "`{tool} -d` ignores such data with a warning; recompress the alignment without it, \
             e.g. `{tool} -dc NAME.{ext} | {tool} > fixed.{ext}`"
    ))
}

fn corrupt(
    source: &Source,
    compression: Option<Compression>,
    detail: &str,
    kind: io::ErrorKind,
) -> DimaError {
    let (name, check) = match compression {
        Some(Compression::Gzip) => ("gzip", "gzip -t"),
        Some(Compression::Bzip2) => ("bzip2", "bzip2 -t"),
        Some(Compression::Zstd) => ("Zstandard", "zstd -t"),
        Some(Compression::None) | None => ("text", ""),
    };
    let what = if kind == io::ErrorKind::UnexpectedEof {
        "truncated"
    } else {
        "corrupt"
    };
    let e = DimaError::input_format(
        Location::new(source.display_name()),
        format!("{what} {name} data: {detail}"),
    );
    if check.is_empty() {
        e
    } else {
        e.with_hint(format!(
            "the file may be truncated or damaged; test it with `{check}` and download or \
             recompress it again"
        ))
    }
}

/// An opened input, ready to be read as batches.
pub struct Opened {
    /// The source.
    pub source: Source,
    /// Compression detected from the content.
    pub compression: Compression,
    /// Size of the stored bytes, for regular files.
    pub stored_size: Option<u64>,
    /// Modification time, for regular files.
    pub modified: Option<SystemTime>,
    /// The batch reader.
    pub batches: BatchReader,
    /// The checksum job; finish it after the batch reader is dropped.
    pub checksum: ChecksumJob,
}

impl Opened {
    /// Drops the reader (closing the checksum stream) and returns the input's identity.
    ///
    /// # Errors
    /// See [`ChecksumJob::finish`].
    pub fn finish(self) -> Result<InputInfo> {
        let Self {
            source,
            compression,
            batches,
            checksum,
            ..
        } = self;
        drop(batches);
        let (checksum, size_bytes) = checksum.finish()?;
        Ok(InputInfo {
            name: source.provenance_name(),
            size_bytes,
            checksum,
            compression,
        })
    }
}

/// Opens `source` for reading.
///
/// # Errors
/// * [`DimaError::Io`] if the source cannot be opened or read (exit 3);
/// * [`DimaError::Usage`] if the path is a directory;
/// * [`DimaError::InputFormat`] for unsupported compression or UTF-16/32 text.
pub fn open(source: &Source, progress: &Arc<Progress>) -> Result<Opened> {
    open_with(source, progress, None)
}

/// [`open`], running `thread_start` first in the checksum thread.
pub(crate) fn open_with(
    source: &Source,
    progress: &Arc<Progress>,
    thread_start: Option<&Hook>,
) -> Result<Opened> {
    let (raw, stored_size, modified) = match source {
        Source::Path(p) => {
            let directory = || {
                DimaError::usage(format!("'{}' is a directory", p.display()))
                    .with_hint("give the path of a FASTA file")
            };
            // Windows refuses to open a directory as a file ("access denied"): say what it is.
            let file = File::open(p).map_err(|e| {
                if p.is_dir() {
                    directory()
                } else {
                    DimaError::io(IoAction::Open, source.target(), e)
                }
            })?;
            let meta = file
                .metadata()
                .map_err(|e| DimaError::io(IoAction::Inspect, source.target(), e))?;
            if meta.is_dir() {
                return Err(directory());
            }
            let size = meta.is_file().then_some(meta.len());
            let modified = meta.is_file().then(|| meta.modified().ok()).flatten();
            (RawHandle::File(Arc::new(file)), size, modified)
        }
        Source::Stdin => (RawHandle::Stdin, None, None),
    };
    if let Some(size) = stored_size {
        progress.set_bytes_total(size);
    }
    let (checksum, tx) = ChecksumJob::spawn(thread_start.cloned())?;
    let mut tee = Tee {
        raw,
        tx,
        progress: Arc::clone(progress),
    };
    let drain = tee.duplicate();
    let mut prefix = [0u8; 6];
    let n = read_up_to(&mut tee, &mut prefix).map_err(|e| map_stream_error(e, source, None))?;
    let prefix = prefix[..n].to_vec();
    let compression = match sniff(&prefix) {
        Sniffed::Supported(c) => c,
        Sniffed::Unsupported(what, hint) => {
            return Err(DimaError::input_format(
                Location::new(source.display_name()).byte(0),
                format!("the input is {what}, which DiMA cannot read"),
            )
            .with_hint(format!(
                "{hint}; DiMA reads plain, gzip/BGZF, bzip2 and Zstandard FASTA"
            )));
        }
    };

    tracing::debug!(
        source = %source.display_name(),
        ?compression,
        stored_size,
        "opened input"
    );
    let chained = BufReader::with_capacity(RAW_BUFFER, Cursor::new(prefix).chain(tee));
    let decoded: Box<dyn Read + Send> = match compression {
        Compression::None => Box::new(chained),
        Compression::Gzip => Box::new(Members::new(
            chained,
            flate2::bufread::GzDecoder::new,
            flate2::bufread::GzDecoder::into_inner,
            &[0x1F, 0x8B],
            "gzip",
        )),
        Compression::Bzip2 => Box::new(Members::new(
            chained,
            bzip2::bufread::BzDecoder::new,
            bzip2::bufread::BzDecoder::into_inner,
            b"BZh",
            "bzip2",
        )),
        Compression::Zstd => {
            let mut d = zstd::stream::read::Decoder::with_buffer(chained)
                .map_err(|e| map_stream_error(e, source, Some(compression)))?;
            // Accept frames written with `--long=31` (up to 2 GiB windows).
            d.window_log_max(31)
                .map_err(|e| map_stream_error(e, source, Some(compression)))?;
            Box::new(d)
        }
    };
    let (text, bom) = handle_bom(decoded, source, compression)?;
    let (text, start) = skip_blank_lines(text, source, compression, bom)?;
    Ok(Opened {
        source: source.clone(),
        compression,
        stored_size,
        modified,
        batches: BatchReader::new(text, source.clone(), compression, start, Some(drain)),
        checksum,
    })
}

/// A record copied verbatim from the input: `>header` line and sequence lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawRecord {
    /// Range of the whole record in [`RawBatch::data`] (from `>` to the final line feed).
    pub range: Range<usize>,
    /// Offset of the line feed that ends the header, relative to `range.start`.
    pub header_end: usize,
    /// 1-based line number of the header.
    pub line: u64,
    /// 0-based byte offset of the `>` in the decompressed text.
    pub byte: u64,
}

/// A batch of consecutive records sharing one buffer.
#[derive(Clone, Debug, Default)]
pub struct RawBatch {
    /// 1-based number of the first record.
    pub first_record: u64,
    /// Record bytes.
    pub data: Vec<u8>,
    /// Records.
    pub records: Vec<RawRecord>,
}

impl RawBatch {
    /// Empties the batch, keeping its allocations.
    pub fn clear(&mut self) {
        self.data.clear();
        self.records.clear();
        self.first_record = 0;
    }

    /// Number of records.
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// `true` when the batch has no records.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Header line of record `i` (without `>` and line ending).
    #[must_use]
    pub fn header(&self, i: usize) -> &[u8] {
        let r = &self.records[i];
        let h = &self.data[r.range.start + 1..r.range.start + r.header_end];
        h.strip_suffix(b"\r").unwrap_or(h)
    }

    /// Sequence bytes of record `i`, line endings included, and the offset of its first byte
    /// relative to the record start.
    #[must_use]
    pub fn sequence(&self, i: usize) -> (&[u8], usize) {
        let r = &self.records[i];
        let start = (r.range.start + r.header_end + 1).min(r.range.end);
        (&self.data[start..r.range.end], start - r.range.start)
    }
}

/// Limits of the batches after the first.
///
/// The first batch is the probe (D40) and is always defined by the documented constants: up to
/// 10,000 records, closing early once it reaches 64 MiB *and* holds at least 1,000 records (or
/// the whole input). Later batches only bound memory, so changing these limits never changes a
/// result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatchLimits {
    /// Maximum records per batch (at least 1).
    pub max_records: usize,
    /// A batch closes once its data reach this size.
    pub max_bytes: usize,
}

impl Default for BatchLimits {
    fn default() -> Self {
        Self {
            max_records: BATCH_MAX_RECORDS,
            max_bytes: BATCH_MAX_BYTES,
        }
    }
}

/// seq_io buffer policy: double without a cap (records of any size are accepted).
struct Doubling;

impl seq_io::policy::BufPolicy for Doubling {
    fn grow_to(&mut self, current_size: usize) -> Option<usize> {
        current_size.checked_mul(2)
    }
}

/// Splits the text into batches of records.
pub struct BatchReader {
    reader: seq_io::fasta::Reader<Box<dyn Read + Send>, Doubling>,
    source: Source,
    compression: Compression,
    byte_base: u64,
    /// Blank lines skipped before the parser's first line.
    line_base: u64,
    next_record: u64,
    limits: BatchLimits,
    finished: bool,
    drain: Option<Tee>,
}

impl BatchReader {
    fn new(
        text: Box<dyn Read + Send>,
        source: Source,
        compression: Compression,
        start: TextStart,
        drain: Option<Tee>,
    ) -> Self {
        Self {
            reader: seq_io::fasta::Reader::with_capacity(text, 1 << 16).set_policy(Doubling),
            source,
            compression,
            byte_base: start.bytes,
            line_base: start.lines,
            next_record: 1,
            limits: BatchLimits::default(),
            finished: false,
            drain,
        }
    }

    /// Replaces the batch limits (tests use tiny batches to exercise batch boundaries).
    #[must_use]
    pub fn with_limits(mut self, limits: BatchLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Records read so far.
    #[must_use]
    pub fn records_read(&self) -> u64 {
        self.next_record - 1
    }

    /// `true` once the end of the input has been reached.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.finished
    }

    /// The input's display name.
    #[must_use]
    pub fn source_name(&self) -> String {
        self.source.display_name()
    }

    fn location(&self, line: u64, byte: u64) -> Location {
        Location::new(self.source.display_name())
            .line(line)
            .byte(byte)
    }

    /// Reads the next batch into `batch` (cleared first). Returns `Ok(false)` once the input is
    /// exhausted and every stored byte has been read (so the checksum is complete).
    ///
    /// # Errors
    /// Structural FASTA errors ([`DimaError::InputFormat`]), I/O errors, corrupt compressed
    /// data, an empty input, or [`DimaError::Cancelled`].
    pub fn read_batch(&mut self, batch: &mut RawBatch, cancel: &AtomicBool) -> Result<bool> {
        batch.clear();
        batch.first_record = self.next_record;
        if self.finished {
            return Ok(false);
        }
        let is_first = self.next_record == 1;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(DimaError::Cancelled);
            }
            let start = batch.data.len();
            let Some(next) = self.reader.next() else {
                self.finish_input()?;
                break;
            };
            match next {
                Ok(record) => {
                    // `write_unchanged` copies the record as stored (adding a final line feed if
                    // the file lacks one); writing to a Vec cannot fail.
                    let _ = record.write_unchanged(&mut batch.data);
                }
                Err(e) => return Err(self.map_parse_error(e)),
            }
            let position = self
                .reader
                .position()
                .ok_or_else(|| DimaError::internal("record without a position"))?;
            let (line, byte) = (
                position.line() + self.line_base,
                position.byte() + self.byte_base,
            );
            let raw = &batch.data[start..];
            let header_end = memchr::memchr(b'\n', raw).unwrap_or(raw.len());
            self.check_header(&raw[..header_end], line, byte)?;
            batch.records.push(RawRecord {
                range: start..batch.data.len(),
                header_end,
                line,
                byte,
            });
            self.next_record += 1;
            let n = batch.records.len();
            let (max_records, max_bytes, min_records) = if is_first {
                (BATCH_MAX_RECORDS, BATCH_MAX_BYTES, PROBE_MIN_RECORDS)
            } else {
                (self.limits.max_records.max(1), self.limits.max_bytes, 1)
            };
            if n >= max_records || (batch.data.len() >= max_bytes && n >= min_records) {
                break;
            }
        }
        if batch.is_empty() {
            if is_first {
                return Err(DimaError::input_format(
                    Location::new(self.source.display_name()),
                    "the input contains no FASTA records",
                )
                .with_hint(
                    "DiMA reads an aligned FASTA file: records start with a '>' header line",
                ));
            }
            return Ok(false);
        }
        Ok(true)
    }

    fn check_header(&self, header_line: &[u8], line: u64, byte: u64) -> Result<()> {
        let body = header_line.strip_suffix(b"\r").unwrap_or(header_line);
        let at = |i: usize| {
            self.location(line, byte + i as u64)
                .record(self.next_record)
        };
        if let Some(i) = memchr::memchr(b'\r', body) {
            // With classic Mac OS line endings (CR only) the whole file is its first line;
            // elsewhere a CR is a stray character, and turning it into a line break would
            // split the header.
            let hint = if line == 1 {
                "the file probably uses classic Mac OS line endings (CR only); convert them, \
                 e.g. `tr '\\r' '\\n' < in.fasta > out.fasta`"
            } else {
                "remove the stray carriage return (0x0D) from the header, e.g. \
                 `tr -d '\\r' < in.fasta > out.fasta`"
            };
            return Err(
                DimaError::input_format(at(i), "carriage return inside a header line")
                    .with_hint(hint),
            );
        }
        if let Some(i) = memchr::memchr(0, body) {
            return Err(DimaError::input_format(at(i), "NUL byte in a header line")
                .with_hint("the file is binary or corrupt; FASTA is plain text"));
        }
        Ok(())
    }

    /// At the end of the text: reads any stored bytes the decoder did not need, so the checksum
    /// covers the whole input.
    fn finish_input(&mut self) -> Result<()> {
        self.finished = true;
        if let Some(mut drain) = self.drain.take() {
            io::copy(&mut drain, &mut io::sink())
                .map_err(|e| map_stream_error(e, &self.source, Some(self.compression)))?;
        }
        Ok(())
    }

    fn map_parse_error(&self, e: seq_io::fasta::Error) -> DimaError {
        match e {
            seq_io::fasta::Error::Io(e) => {
                map_stream_error(e, &self.source, Some(self.compression))
            }
            seq_io::fasta::Error::InvalidStart { line, found } => {
                let loc =
                    Location::new(self.source.display_name()).line(line as u64 + self.line_base);
                if found == b'@' {
                    DimaError::input_format(loc, "this looks like FASTQ ('@' header)").with_hint(
                        "DiMA analyses aligned FASTA; align the sequences and save them as FASTA",
                    )
                } else {
                    DimaError::input_format(
                        loc,
                        format!(
                            "expected a '>' header line, found {}",
                            crate::normalise::describe_byte(found)
                        ),
                    )
                    .with_hint(start_hint(found, None))
                }
            }
            seq_io::fasta::Error::BufferLimit => {
                DimaError::internal("record buffer limit reached (no limit should apply)")
            }
        }
    }
}

/// Reads a whole regular file's checksum without parsing it (used by tests and by the GUI's
/// change detection).
///
/// # Errors
/// [`DimaError::Io`] if the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<String> {
    let target = || IoTarget::Path(path.to_path_buf());
    let mut f = File::open(path).map_err(|e| DimaError::io(IoAction::Open, target(), e))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; RAW_BUFFER];
    loop {
        let n = match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(DimaError::io(IoAction::Read, target(), e)),
        };
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// `(header, sequence bytes)` pairs.
#[cfg(test)]
pub(crate) type TestRecords = Vec<(Vec<u8>, Vec<u8>)>;

/// Reads the whole input into memory as a single list of records (header, sequence bytes);
/// for tests and small inspections only.
#[cfg(test)]
pub(crate) fn read_all(source: &Source) -> Result<(TestRecords, InputInfo)> {
    let progress = Arc::new(Progress::new());
    let mut opened = open(source, &progress)?;
    let cancel = AtomicBool::new(false);
    let mut out = Vec::new();
    let mut batch = RawBatch::default();
    while opened.batches.read_batch(&mut batch, &cancel)? {
        for i in 0..batch.len() {
            out.push((batch.header(i).to_vec(), batch.sequence(i).0.to_vec()));
        }
    }
    let info = opened.finish()?;
    Ok((out, info))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp(bytes: &[u8], name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        (dir, p)
    }

    fn sha(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    const FASTA: &[u8] = b">a|x\nACGT\nAC\n>b|y\r\nACGTAC\r\n>c\nAC GT-A\n";

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn bzip(data: &[u8]) -> Vec<u8> {
        let mut e = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn zst(data: &[u8]) -> Vec<u8> {
        zstd::encode_all(data, 3).unwrap()
    }

    #[test]
    fn records_and_checksums_for_every_compression() {
        // Each format, and two concatenated streams/members/frames of each.
        let half = FASTA.len() / 2;
        let cases: Vec<(&str, Vec<u8>, Compression)> = vec![
            ("plain", FASTA.to_vec(), Compression::None),
            ("gz", gzip(FASTA), Compression::Gzip),
            (
                "gz2",
                [gzip(&FASTA[..half]), gzip(&FASTA[half..])].concat(),
                Compression::Gzip,
            ),
            (
                "gz-padded",
                [gzip(FASTA), vec![0; 5000]].concat(),
                Compression::Gzip,
            ),
            ("bz2", bzip(FASTA), Compression::Bzip2),
            (
                "bz2x2",
                [bzip(&FASTA[..half]), bzip(&FASTA[half..])].concat(),
                Compression::Bzip2,
            ),
            (
                "bz2-padded",
                [bzip(FASTA), vec![0; 3]].concat(),
                Compression::Bzip2,
            ),
            ("zst", zst(FASTA), Compression::Zstd),
            (
                "zstx2",
                [zst(&FASTA[..half]), zst(&FASTA[half..])].concat(),
                Compression::Zstd,
            ),
        ];
        for (name, bytes, compression) in cases {
            let (_d, p) = tmp(&bytes, name);
            let (records, info) = read_all(&Source::Path(p)).unwrap();
            assert_eq!(info.compression, compression, "{name}");
            assert_eq!(info.size_bytes, bytes.len() as u64, "{name}");
            assert_eq!(info.checksum.value, sha(&bytes), "{name}");
            assert_eq!(info.name, name);
            let heads: Vec<_> = records.iter().map(|r| r.0.clone()).collect();
            assert_eq!(
                heads,
                [b"a|x".to_vec(), b"b|y".to_vec(), b"c".to_vec()],
                "{name}"
            );
            assert_eq!(records[0].1, b"ACGT\nAC\n", "{name}");
            assert_eq!(records[1].1, b"ACGTAC\r\n", "{name}");
            assert_eq!(records[2].1, b"AC GT-A\n", "{name}");
        }
    }

    #[test]
    fn zstd_skippable_frame_and_long_window() {
        let mut skippable = vec![0x50, 0x2A, 0x4D, 0x18, 4, 0, 0, 0, 1, 2, 3, 4];
        skippable.extend(zst(FASTA));
        let (_d, p) = tmp(&skippable, "s.zst");
        let (records, info) = read_all(&Source::Path(p)).unwrap();
        assert_eq!((records.len(), info.compression), (3, Compression::Zstd));
        let mut enc = zstd::stream::write::Encoder::new(Vec::new(), 3).unwrap();
        enc.long_distance_matching(true).unwrap();
        enc.window_log(31).unwrap();
        enc.write_all(FASTA).unwrap();
        let (_d, p) = tmp(&enc.finish().unwrap(), "l.zst");
        assert_eq!(read_all(&Source::Path(p)).unwrap().0.len(), 3);
    }

    #[test]
    fn bom_and_encodings() {
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(FASTA);
        let (_d, p) = tmp(&gzip(&with_bom), "bom.gz");
        let (records, _) = read_all(&Source::Path(p)).unwrap();
        assert_eq!(records[0].0, b"a|x");
        for (bad, encoding) in [
            (&[0xFF, 0xFE, b'>', 0][..], "UTF-16"),
            (&[0xFE, 0xFF, 0, b'>'], "UTF-16"),
            (&[0, 0, 0xFE, 0xFF], "UTF-32"),
            (&[0xFF, 0xFE, 0, 0], "UTF-32"),
        ] {
            let (_d, p) = tmp(bad, "u16");
            let e = read_all(&Source::Path(p)).err().unwrap();
            assert!(
                e.to_string().contains(&format!("is {encoding} encoded")),
                "{e}"
            );
            assert!(e.hint().unwrap().contains(&format!("iconv -f {encoding}")));
        }
    }

    #[test]
    fn unsupported_formats_are_named() {
        for (bytes, what) in [
            (&[0x1F, 0x9D, 0x90, 0x3E][..], ".Z"),
            (&[0x04, 0x22, 0x4D, 0x18, 0, 0], "LZ4"),
            (&[0x5D, 0, 0, 0x80, 0, 0], "LZMA"),
            (b"PK\x03\x04abc", "zip"),
            (&[b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C], "7z"),
            (&[0xFD, b'7', b'z', b'X', b'Z', 0x00, 0x00, 0x04], "xz"),
        ] {
            let (_d, p) = tmp(bytes, "x");
            let e = read_all(&Source::Path(p)).err().unwrap();
            assert!(e.to_string().contains(what), "{e}");
            assert_eq!(e.class(), crate::error::ExitClass::InvalidInput);
            let hint = e.hint().unwrap();
            assert!(
                hint.ends_with("DiMA reads plain, gzip/BGZF, bzip2 and Zstandard FASTA"),
                "{hint}"
            );
        }
        // xz is refused with the ways to read it (D116).
        let (_d, p) = tmp(&[0xFD, b'7', b'z', b'X', b'Z', 0x00], "a.fasta.xz");
        let e = read_all(&Source::Path(p)).err().unwrap();
        assert!(
            e.to_string()
                .contains("the input is xz (.xz), which DiMA cannot read"),
            "{e}"
        );
        assert!(e.hint().unwrap().contains("`xz -dk NAME.xz`"));
        assert!(e.hint().unwrap().contains("zstd"));
    }

    #[test]
    fn structural_errors() {
        let check = |bytes: &[u8], needle: &str| {
            let (_d, p) = tmp(bytes, "f.fa");
            let e = read_all(&Source::Path(p))
                .err()
                .unwrap_or_else(|| panic!("{needle}: no error"));
            assert!(e.to_string().contains(needle), "{needle}: {e}");
            assert_eq!(e.class(), crate::error::ExitClass::InvalidInput, "{needle}");
            e
        };
        let e = check(b"", "no FASTA records");
        assert!(e.hint().is_some());
        // Blank and white-space-only lines before the first record are skipped.
        let e = check(b"\n\n  \n\t\x0B\x0C\r\n \t", "no FASTA records");
        assert!(e.hint().unwrap().contains("'>' header line"), "{e}");
        let e = check(b"ACGT\n>a\nACGT\n", "expected a '>' header line");
        assert!(e.to_string().contains("line 1"));
        check(b"@r1\nACGT\n+\nIIII\n", "FASTQ");
        let e = check(b">a\rACGT\r>b\rACGT\r", "carriage return inside a header");
        assert!(e.hint().unwrap().contains("classic Mac OS"), "{e}");
        let e = check(
            b">a\nACGT\n>b\nACGT\n>c\rd\nACGT\n",
            "carriage return inside a header",
        );
        assert!(e.to_string().contains("record 3"), "{e}");
        assert!(e.hint().unwrap().contains("tr -d"), "{e}");
        let e = check(b">a\x00b\nACGT\n", "NUL byte in a header");
        assert!(e.to_string().contains("record 1"), "{e}");
        let e = check(b" \n\tx\n>a\nACGT\n", "found 'x'");
        assert!(e.to_string().contains("line 2, column 2, byte 3"), "{e}");
        // The byte is shown plainly, never escaped (the path before it may hold backslashes).
        let text = e.to_string();
        assert!(!text[text.find("found").unwrap()..].contains('\\'), "{e}");
        let e = check(
            b"\n  >a\nACGT\n",
            "white space before the first header's '>'",
        );
        assert!(e.to_string().contains("line 2, column 3, byte 3"), "{e}");
        assert!(e.hint().unwrap().contains("remove the white space"), "{e}");
        // A no-break space is not white space; at a line start or after white space.
        for text in [&b"\xC2\xA0>a\nAC\n"[..], b" \xC2\xA0>a\nAC\n"] {
            let e = check(text, "found 0xC2");
            assert!(e.hint().unwrap().contains("no-break space"), "{e}");
        }
        let e = check(b"\x00\x00", "found 0x00");
        assert!(e.hint().unwrap().contains("binary"), "{e}");
        let truncated = gzip(FASTA);
        let e = check(&truncated[..truncated.len() - 8], "gzip data");
        assert!(e.hint().unwrap().contains("gzip -t"));
        // Damage found while decoding the first block names the compression too.
        let mut bz = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::best());
        bz.write_all(FASTA).unwrap();
        let bz = bz.finish().unwrap();
        let e = check(&bz[..bz.len() / 2], "truncated bzip2 data");
        assert!(e.hint().unwrap().contains("bzip2 -t"), "{e}");
        let zst = zstd::encode_all(FASTA, 3).unwrap();
        let e = check(&zst[..zst.len() / 2], "Zstandard data");
        assert!(e.hint().unwrap().contains("zstd -t"), "{e}");
        let e = check(&[0x1F, 0x8B], "truncated gzip data");
        assert!(e.hint().unwrap().contains("gzip -t"), "{e}");
        // UTF-32 is named as such (UTF-16's conversion would not help).
        let mut utf32 = vec![0xFF, 0xFE, 0x00, 0x00];
        utf32.extend(">a\nAC\n".bytes().flat_map(|b| [b, 0, 0, 0]));
        let e = check(&utf32, "UTF-32");
        assert!(e.hint().unwrap().contains("iconv -f UTF-32"), "{e}");
        let e = check(&[0xFE, 0xFF, 0, b'>'], "UTF-16");
        assert!(e.hint().unwrap().contains("iconv -f UTF-16"), "{e}");
        // Data after the compressed data that is not zero padding.
        let mut junk = gzip(FASTA);
        junk.extend(b"\0\0junk");
        let e = check(&junk, "6 bytes that are not gzip data");
        assert!(e.hint().unwrap().contains("gzip -dc"), "{e}");
    }

    #[test]
    fn missing_input_and_directories() {
        let e = read_all(&Source::Path("/nonexistent/x.fa".into()))
            .err()
            .unwrap();
        assert_eq!(e.class(), crate::error::ExitClass::Io);
        assert!(e.to_string().contains("cannot open '/nonexistent/x.fa'"));
        let dir = tempfile::tempdir().unwrap();
        let e = read_all(&Source::Path(dir.path().to_path_buf()))
            .err()
            .unwrap();
        assert_eq!(e.class(), crate::error::ExitClass::Usage);
    }

    #[test]
    fn locations_after_many_blank_lines() {
        // More blank lines than one parser buffer holds (64 KiB), LF and CR LF.
        for (blank, width) in [(&b"\n"[..], 1u64), (&b"\r\n"[..], 2)] {
            let mut text = blank.repeat(70_000);
            text.extend_from_slice(b">a\nACGTACGTACGT\n>b\nACGTACGTAC1T\n");
            let (_d, p) = tmp(&text, "blank.fa");
            let mut opened = open(&Source::Path(p), &Arc::new(Progress::new())).unwrap();
            let mut batch = RawBatch::default();
            opened
                .batches
                .read_batch(&mut batch, &AtomicBool::new(false))
                .unwrap();
            let lines: Vec<u64> = batch.records.iter().map(|r| r.line).collect();
            let bytes: Vec<u64> = batch.records.iter().map(|r| r.byte).collect();
            assert_eq!(lines, [70_001, 70_003]);
            assert_eq!(bytes, [70_000 * width, 70_000 * width + 16]);
        }
        // White-space-only lines, and one line of white space longer than a buffer: counted
        // exactly, never kept.
        for (lines, count, width) in [
            (&b" \t\x0B\x0C\r\n"[..], 70_000u64, 6u64),
            (&[b' '; 100_000][..], 1, 100_000),
        ] {
            let mut text = lines.repeat(usize::try_from(count).unwrap());
            if count == 1 {
                text.push(b'\n');
            }
            text.extend_from_slice(b">a\nACGT\n");
            let (_d, p) = tmp(&text, "ws.fa");
            let mut opened = open(&Source::Path(p), &Arc::new(Progress::new())).unwrap();
            let mut batch = RawBatch::default();
            opened
                .batches
                .read_batch(&mut batch, &AtomicBool::new(false))
                .unwrap();
            let skipped = if count == 1 { width + 1 } else { count * width };
            assert_eq!(
                (batch.records[0].line, batch.records[0].byte),
                (count + 1, skipped)
            );
        }
        let mut text = vec![b' '; 100_000];
        text.extend_from_slice(b"x\n>a\nAC\n");
        let (_d, p) = tmp(&text, "ws.fa");
        let e = read_all(&Source::Path(p)).err().unwrap();
        assert!(
            e.to_string().contains("line 1, column 100001, byte 100000"),
            "{e}"
        );
        // Blank lines only: no records; a lone CR before content is refused.
        let (_d, p) = tmp(&b"\n".repeat(70_000), "empty.fa");
        let e = read_all(&Source::Path(p)).err().unwrap();
        assert!(e.to_string().contains("no FASTA records"), "{e}");
        let (_d, p) = tmp(b"\n\r>a\nAC\n", "cr.fa");
        let e = read_all(&Source::Path(p)).err().unwrap();
        assert!(e.to_string().contains("line 2"), "{e}");
        assert!(e.hint().unwrap().contains("classic Mac OS"), "{e}");
        // Classic Mac OS line endings are found at the start, not after the whole file.
        for (start, at) in [
            (&b"\xEF\xBB\xBF\n>a\r"[..], "record 1 (line 2, byte 6)"),
            (b">MERS_spike|Camel\r", "record 1 (line 1, byte 17)"),
        ] {
            let mut mac = start.to_vec();
            mac.extend(b"ACGT\r".repeat(100_000));
            let (_d, p) = tmp(&mac, "mac.fa");
            let e = read_all(&Source::Path(p)).err().unwrap();
            assert!(e.to_string().contains(at), "{e}");
            assert!(e.hint().unwrap().contains("classic Mac OS"), "{e}");
        }
    }

    #[test]
    fn positions_and_batches() {
        // 10,000 records form the first batch (the probe); later batches follow the limits.
        let mut text = Vec::new();
        for i in 0..10_007 {
            write!(text, ">r{i}\r\nAC\r\nGT\r\n").unwrap();
        }
        let (_d, p) = tmp(&text, "b.fa");
        let progress = Arc::new(Progress::new());
        let opened = open(&Source::Path(p), &progress).unwrap();
        let mut reader = opened.batches.with_limits(BatchLimits {
            max_records: 3,
            max_bytes: usize::MAX,
        });
        let cancel = AtomicBool::new(false);
        let mut batch = RawBatch::default();
        let mut sizes = Vec::new();
        let mut lines = Vec::new();
        while reader.read_batch(&mut batch, &cancel).unwrap() {
            sizes.push((batch.first_record, batch.len()));
            for (i, r) in batch.records.iter().enumerate() {
                lines.push((r.line, r.byte));
                let n = batch.first_record as usize - 1 + i;
                assert_eq!(batch.header(i), format!("r{n}").as_bytes());
                assert_eq!(batch.sequence(i).0, b"AC\r\nGT\r\n");
            }
        }
        assert_eq!(sizes, [(1, 10_000), (10_001, 3), (10_004, 3), (10_007, 1)]);
        assert_eq!(lines[0], (1, 0));
        assert_eq!(lines[1], (4, 13));
        assert_eq!(reader.records_read(), 10_007);
        assert!(reader.is_finished());
        // Cancellation stops reading.
        let (_d, p) = tmp(FASTA, "c.fa");
        let mut opened = open(&Source::Path(p), &progress).unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(matches!(
            opened.batches.read_batch(&mut batch, &cancel),
            Err(DimaError::Cancelled)
        ));
    }

    #[test]
    fn first_batch_is_the_probe_whatever_the_limits() {
        let mut text = Vec::new();
        for i in 0..1_050 {
            write!(text, ">r{i}\n{}\n", "A".repeat(100)).unwrap();
        }
        let (_d, p) = tmp(&text, "m.fa");
        let progress = Arc::new(Progress::new());
        let opened = open(&Source::Path(p), &progress).unwrap();
        let mut reader = opened.batches.with_limits(BatchLimits {
            max_records: 1000,
            max_bytes: 10,
        });
        let cancel = AtomicBool::new(false);
        let mut batch = RawBatch::default();
        assert!(reader.read_batch(&mut batch, &cancel).unwrap());
        assert_eq!(
            batch.len(),
            1_050,
            "the probe ignores the limits of later batches"
        );
    }

    #[test]
    fn query_names() {
        let q = |s: &str| query_name_from_source(&Source::Path(s.into()));
        assert_eq!(
            q("data/HIV1_FLT_2022_pol_DNA.fasta.bz2").as_deref(),
            Some("HIV1_FLT_2022_pol_DNA")
        );
        assert_eq!(q("x/MERS.Spike.FA.GZ").as_deref(), Some("MERS.Spike"));
        assert_eq!(q("a.afa").as_deref(), Some("a"));
        assert_eq!(q("noext").as_deref(), Some("noext"));
        assert_eq!(q(".fasta"), None);
        assert_eq!(q("x.fa.fasta.gz").as_deref(), Some("x"));
        // Recompressed from xz as the hint for xz input suggests.
        assert_eq!(q("spike.fasta.xz.zst").as_deref(), Some("spike"));
        assert_eq!(query_name_from_source(&Source::Stdin), None);
        assert_eq!(Source::Path("d/e/f.fa".into()).provenance_name(), "f.fa");
        assert_eq!(Source::Stdin.provenance_name(), "-");
        // Process substitution and descriptor paths are named like standard input: their
        // numbers are the shell's.
        for alias in [
            "/dev/fd/63",
            "/dev/stdin",
            "/proc/self/fd/0",
            "/proc/4242/fd/11",
        ] {
            let s = Source::Path(alias.into());
            assert_eq!(
                query_name_from_source(&s),
                if cfg!(unix) {
                    None
                } else {
                    Some(alias.rsplit('/').next().unwrap().to_owned())
                },
                "{alias}"
            );
            if cfg!(unix) {
                assert_eq!(s.provenance_name(), "-", "{alias}");
                assert_eq!(
                    crate::run_report::file_name(std::path::Path::new(alias)),
                    "-"
                );
            }
        }
        assert_eq!(
            q("/dev/fdx/63.fa"),
            Some("63".into()),
            "only the descriptor paths"
        );
    }

    #[test]
    fn sha256_of_file() {
        let (_d, p) = tmp(b"abc", "h");
        assert_eq!(
            sha256_file(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[cfg(unix)]
    #[test]
    fn fifo_input() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("pipe");
        let status = std::process::Command::new("mkfifo")
            .arg(&p)
            .status()
            .unwrap();
        assert!(status.success());
        let writer_path = p.clone();
        let data = gzip(FASTA);
        let expected = sha(&data);
        let t = thread::spawn(move || {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .open(writer_path)
                .unwrap();
            // Short writes exercise the sniffing loop on pipes.
            for chunk in data.chunks(3) {
                f.write_all(chunk).unwrap();
                f.flush().unwrap();
            }
        });
        let (records, info) = read_all(&Source::Path(p)).unwrap();
        t.join().unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(info.checksum.value, expected);
        assert_eq!(info.compression, Compression::Gzip);
    }
}
