//! Atomic output files (D77; finding E2).
//!
//! A [`Sink`] writes to a temporary file in the destination's directory and, on
//! [`Sink::commit`], flushes the encoder, syncs the file, renames it over the destination and
//! syncs the directory: readers see either the old file or the complete new one, never a
//! partial file, and an interrupted run leaves nothing behind (the temporary file is removed when
//! the sink is dropped uncommitted). A new file gets the usual `0666 & !umask` permissions; a
//! replaced file keeps its permission bits on Unix, so a file made private (results can hold
//! header metadata) stays private. A symbolic link is resolved first, so
//! the link keeps pointing at the (replaced) target. Standard output and destinations that are
//! not regular files (`/dev/stdout`, FIFOs) are written directly.
//!
//! A sink created with a cancellation flag stops at the next buffer it hands to the destination
//! once the flag is set ([`DimaError::Cancelled`]), so a long write never outlasts an
//! interruption. A sink dropped uncommitted discards what it still buffers: a failed output
//! never ends with a flushed fragment (on standard output, nothing more is written).

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use crate::error::{DimaError, IoAction, IoTarget, Result};

/// Buffer between the encoder and the file.
const BUFFER: usize = 1 << 20;
/// Buffer in front of a compressor: serialisers write many small pieces, and every write into
/// a compressor runs it (flate2 compresses on each call).
const INPUT_BUFFER: usize = 1 << 16;
/// Characters of the destination's name kept in its temporary file's name, which must stay
/// within the file system's name limit (255 bytes on most).
const TEMP_NAME_CHARS: usize = 64;

/// Where output goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Standard output.
    Stdout,
    /// A file.
    Path(PathBuf),
}

impl Target {
    /// `-` means standard output.
    #[must_use]
    pub fn from_arg(arg: &Path) -> Self {
        if arg.as_os_str() == "-" {
            Self::Stdout
        } else {
            Self::Path(arg.to_path_buf())
        }
    }

    fn io_target(&self) -> IoTarget {
        match self {
            Self::Stdout => IoTarget::Stdout,
            Self::Path(p) => IoTarget::Path(p.clone()),
        }
    }
}

/// Stream compression of an output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OutputCompression {
    /// Plain text.
    None,
    /// gzip (level 6).
    Gzip,
    /// Zstandard (level 3).
    Zstd,
}

enum Raw {
    Stdout(io::Stdout),
    File(File),
}

impl Write for Raw {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Stdout(s) => s.write(buf),
            Self::File(f) => f.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Stdout(s) => s.flush(),
            Self::File(f) => f.flush(),
        }
    }
}

/// Marks the error that stops a cancelled write.
#[derive(Debug)]
struct WriteCancelled;

impl std::fmt::Display for WriteCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for WriteCancelled {}

/// Counts and hashes the bytes that reach the destination (after compression); stops when
/// cancelled, and drops everything once the sink is discarded.
struct Counted {
    raw: Raw,
    hasher: Sha256,
    bytes: u64,
    cancel: Option<Arc<AtomicBool>>,
    discard: Arc<AtomicBool>,
}

impl Write for Counted {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.discard.load(Ordering::Relaxed) {
            return Ok(buf.len());
        }
        if self
            .cancel
            .as_ref()
            .is_some_and(|c| c.load(Ordering::Relaxed))
        {
            return Err(io::Error::other(WriteCancelled));
        }
        let n = self.raw.write(buf)?;
        self.hasher.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.discard.load(Ordering::Relaxed) {
            return Ok(());
        }
        self.raw.flush()
    }
}

/// Gives a temporary file the permission bits of the file it will replace, if there is one
/// (Unix; best effort: the default permissions remain if this fails).
#[cfg(unix)]
fn keep_permissions(dest: &Path, file: &File) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::metadata(dest) {
        if meta.is_file() {
            let mode = meta.permissions().mode() & 0o777;
            let _ = file.set_permissions(fs::Permissions::from_mode(mode));
        }
    }
}

#[cfg(not(unix))]
fn keep_permissions(_dest: &Path, _file: &File) {}

/// A write error with the destination, or [`DimaError::Cancelled`].
fn write_error(target: &Target, action: IoAction, e: io::Error) -> DimaError {
    if e.get_ref().is_some_and(|i| i.is::<WriteCancelled>()) {
        DimaError::Cancelled
    } else {
        DimaError::io(action, target.io_target(), e)
    }
}

/// What a committed sink wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    /// Bytes written (after compression).
    pub bytes: u64,
    /// SHA-256 of those bytes, lower-case hexadecimal.
    pub sha256: String,
}

enum Encoder {
    Plain(BufWriter<Counted>),
    Gzip(BufWriter<flate2::write::GzEncoder<BufWriter<Counted>>>),
    Zstd(BufWriter<zstd::stream::write::Encoder<'static, BufWriter<Counted>>>),
}

impl Encoder {
    fn finish(self) -> io::Result<Counted> {
        let buffered = match self {
            Self::Plain(w) => w,
            Self::Gzip(w) => w
                .into_inner()
                .map_err(io::IntoInnerError::into_error)?
                .finish()?,
            Self::Zstd(w) => w
                .into_inner()
                .map_err(io::IntoInnerError::into_error)?
                .finish()?,
        };
        buffered
            .into_inner()
            .map_err(io::IntoInnerError::into_error)
    }
}

/// An output being written; see the module documentation.
pub struct Sink {
    target: Target,
    /// Final destination (symlinks resolved) and temporary file, when writing atomically.
    atomic: Option<(PathBuf, PathBuf)>,
    encoder: Option<Encoder>,
    /// Set when the sink is dropped uncommitted: nothing more reaches the destination.
    discard: Arc<AtomicBool>,
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Temporary files of sinks that are still being written.
static LIVE_TEMPORARIES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

fn live() -> std::sync::MutexGuard<'static, Vec<PathBuf>> {
    LIVE_TEMPORARIES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Removes the temporary files of every sink still being written. For front ends that must
/// exit immediately (a second interrupt) and cannot unwind: the destinations are left as they
/// were.
pub fn remove_temporaries() {
    for p in live().drain(..) {
        let _ = fs::remove_file(p);
    }
}

/// Where and how a path's data is written.
#[derive(Debug, PartialEq, Eq)]
enum Destination {
    /// A regular file (existing or not), replaced atomically; symbolic links are followed so
    /// the link's target is replaced, not the link.
    Atomic(PathBuf),
    /// Written in place: devices and FIFOs, and (with `append`) the aliases of open
    /// descriptors such as `/dev/stdout`, which may be a file the shell opened for appending
    /// (`>> log`) and must never be truncated or replaced.
    Direct { path: PathBuf, append: bool },
}

/// Resolves `path` one symbolic link at a time (canonicalising would turn `/dev/stdout` into
/// the file behind the descriptor and replace it).
fn resolve_destination(path: &Path) -> Destination {
    let mut current = path.to_path_buf();
    for _ in 0..40 {
        if crate::input::is_descriptor_alias(&current) {
            return Destination::Direct {
                path: path.to_path_buf(),
                append: true,
            };
        }
        match fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_symlink() => match fs::read_link(&current) {
                Ok(target) if target.is_absolute() => current = target,
                Ok(target) => {
                    let dir = current.parent().map(Path::to_path_buf).unwrap_or_default();
                    current = dir.join(target);
                }
                Err(_) => break,
            },
            Ok(m) if !m.is_file() => {
                return Destination::Direct {
                    path: current,
                    append: false,
                };
            }
            _ => break,
        }
    }
    Destination::Atomic(current)
}

/// The file an output to `path` is written to: symbolic links followed one at a time, dangling
/// ones included (the link's target is created), and descriptor aliases such as `/dev/stdout`
/// kept as given. Two outputs with the same destination would replace each other.
#[must_use]
pub fn destination_of(path: &Path) -> PathBuf {
    match resolve_destination(path) {
        Destination::Atomic(p) | Destination::Direct { path: p, .. } => p,
    }
}

/// `true` when an output to `path` is written in place, as a stream, rather than through a
/// temporary file renamed over it: a device such as `/dev/null`, a FIFO, a descriptor alias
/// such as `/dev/fd/63` (from `>(…)`), or a symbolic link to one. Planners ask this to treat
/// such names as streams exactly as [`Sink`] will write them. A folder is neither (planners
/// refuse it).
#[must_use]
pub fn written_in_place(path: &Path) -> bool {
    match resolve_destination(path) {
        Destination::Direct { path, .. } => !path.is_dir(),
        Destination::Atomic(_) => false,
    }
}

/// Why no file can be written at `dest`, known before writing: a symbolic-link loop (the
/// resolution gave up on a link), or a file name longer than any file system allows (more
/// than 255 bytes on Linux, 255 UTF-16 units elsewhere), which would otherwise fail only when
/// the finished output is renamed into place.
fn unusable_name(dest: &Path) -> Option<io::Error> {
    if fs::symlink_metadata(dest).is_ok_and(|m| m.file_type().is_symlink()) {
        return Some(io::Error::other("too many levels of symbolic links"));
    }
    let name = dest.file_name()?;
    let too_long = if cfg!(target_os = "linux") {
        name.len() > NAME_MAX
    } else {
        name.to_string_lossy().encode_utf16().count() > NAME_MAX
    };
    too_long.then(|| {
        io::Error::other(format!(
            "file name too long (more than {NAME_MAX} {})",
            if cfg!(target_os = "linux") {
                "bytes"
            } else {
                "characters"
            }
        ))
    })
}

/// The longest file name file systems accept: 255 bytes on Linux (its `NAME_MAX`), 255 UTF-16
/// units on NTFS and HFS+.
const NAME_MAX: usize = 255;

impl Sink {
    /// Opens `target` for writing with the given compression.
    ///
    /// # Errors
    /// [`DimaError::Io`] if the temporary file (or the destination) cannot be created.
    pub fn create(target: &Target, compression: OutputCompression) -> Result<Self> {
        Self::create_cancellable(target, compression, None)
    }

    /// [`Sink::create`] with a cancellation flag: once it is set, the next write that reaches
    /// the destination fails with [`DimaError::Cancelled`].
    ///
    /// # Errors
    /// As [`Sink::create`].
    pub fn create_cancellable(
        target: &Target,
        compression: OutputCompression,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<Self> {
        let io_err = |action, e| DimaError::io(action, target.io_target(), e);
        let (raw, atomic) = match target {
            Target::Stdout => (Raw::Stdout(io::stdout()), None),
            Target::Path(path) => match resolve_destination(path) {
                Destination::Direct { path, append } => {
                    let f = OpenOptions::new()
                        .write(true)
                        .append(append)
                        .open(&path)
                        .map_err(|e| io_err(IoAction::Open, e))?;
                    (Raw::File(f), None)
                }
                Destination::Atomic(dest) => {
                    if let Some(e) = unusable_name(&dest) {
                        return Err(io_err(IoAction::Create, e));
                    }
                    let dir = match dest.parent() {
                        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
                        _ => PathBuf::from("."),
                    };
                    let name: String = dest
                        .file_name()
                        .map_or_else(|| "output".into(), |n| n.to_string_lossy().into_owned())
                        .chars()
                        .take(TEMP_NAME_CHARS)
                        .collect();
                    let (file, temp) = loop {
                        let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
                        let temp = dir.join(format!(".{name}.dima-{}-{n}.tmp", std::process::id()));
                        match OpenOptions::new().write(true).create_new(true).open(&temp) {
                            Ok(f) => {
                                live().push(temp.clone());
                                keep_permissions(&dest, &f);
                                break (f, temp);
                            }
                            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                            Err(e) => return Err(io_err(IoAction::Create, e)),
                        }
                    };
                    (Raw::File(file), Some((dest, temp)))
                }
            },
        };
        let discard = Arc::new(AtomicBool::new(false));
        let buffered = BufWriter::with_capacity(
            BUFFER,
            Counted {
                raw,
                hasher: Sha256::new(),
                bytes: 0,
                cancel,
                discard: Arc::clone(&discard),
            },
        );
        let encoder = match compression {
            OutputCompression::None => Encoder::Plain(buffered),
            OutputCompression::Gzip => Encoder::Gzip(BufWriter::with_capacity(
                INPUT_BUFFER,
                flate2::write::GzEncoder::new(buffered, flate2::Compression::default()),
            )),
            OutputCompression::Zstd => Encoder::Zstd(BufWriter::with_capacity(
                INPUT_BUFFER,
                zstd::stream::write::Encoder::new(buffered, 3)
                    .map_err(|e| io_err(IoAction::Create, e))?,
            )),
        };
        Ok(Self {
            target: target.clone(),
            atomic,
            encoder: Some(encoder),
            discard,
        })
    }

    /// The destination.
    #[must_use]
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// Wraps a write error with the destination ([`DimaError::Cancelled`] after a
    /// cancellation).
    #[must_use]
    pub fn error(&self, e: io::Error) -> DimaError {
        write_error(&self.target, IoAction::Write, e)
    }

    /// Finishes the output: flushes, syncs and atomically replaces the destination.
    ///
    /// # Errors
    /// [`DimaError::Io`] with the failed step.
    pub fn commit(mut self) -> Result<Written> {
        let target = self.target.io_target();
        let encoder = self.encoder.take().expect("committed once");
        let counted = encoder
            .finish()
            .map_err(|e| write_error(&self.target, IoAction::Write, e))?;
        let written = Written {
            bytes: counted.bytes,
            sha256: counted
                .hasher
                .finalize()
                .iter()
                .fold(String::with_capacity(64), |mut s, b| {
                    use std::fmt::Write as _;
                    let _ = write!(s, "{b:02x}");
                    s
                }),
        };
        match counted.raw {
            Raw::Stdout(mut s) => s
                .flush()
                .map(|()| written)
                .map_err(|e| DimaError::io(IoAction::Write, target, e)),
            Raw::File(f) => {
                // Only atomically written regular files are synced and renamed; FIFOs and
                // devices cannot be synced (EINVAL) and are written in place.
                if let Some((dest, temp)) = self.atomic.take() {
                    // The temporary file stays registered until it has been renamed, so a
                    // forced exit at any point before that still removes it.
                    let unregister = || live().retain(|p| *p != temp);
                    if let Err(e) = f.sync_all() {
                        drop(f);
                        let _ = fs::remove_file(&temp);
                        unregister();
                        return Err(DimaError::io(IoAction::Sync, target, e));
                    }
                    drop(f);
                    if let Err(e) = fs::rename(&temp, &dest) {
                        let _ = fs::remove_file(&temp);
                        unregister();
                        return Err(DimaError::io(IoAction::Replace, target, e));
                    }
                    unregister();
                    sync_parent(&dest);
                }
                tracing::debug!(target = %target, bytes = written.bytes, "output committed");
                Ok(written)
            }
        }
    }
}

/// Makes the rename durable (POSIX); a no-op elsewhere.
fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(dir) = path.parent() {
        let dir = if dir.as_os_str().is_empty() {
            Path::new(".")
        } else {
            dir
        };
        if let Ok(d) = File::open(dir) {
            let _ = d.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.encoder.as_mut().expect("not committed") {
            Encoder::Plain(w) => w.write(buf),
            Encoder::Gzip(w) => w.write(buf),
            Encoder::Zstd(w) => w.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.encoder.as_mut().expect("not committed") {
            Encoder::Plain(w) => w.flush(),
            Encoder::Gzip(w) => w.flush(),
            Encoder::Zstd(w) => w.flush(),
        }
    }
}

impl Drop for Sink {
    fn drop(&mut self) {
        if self.encoder.is_some() {
            // Uncommitted: what is still buffered (and what dropping the encoder would flush or
            // finish) never reaches the destination.
            self.discard.store(true, Ordering::Relaxed);
        }
        // Close the file before removing it (required on Windows).
        drop(self.encoder.take());
        if let Some((_, temp)) = self.atomic.take() {
            live().retain(|p| *p != temp);
            let _ = fs::remove_file(temp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn entries(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn a_cancelled_sink_stops_and_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("big.json");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut sink = Sink::create_cancellable(
            &Target::Path(dest.clone()),
            OutputCompression::None,
            Some(Arc::clone(&cancel)),
        )
        .unwrap();
        sink.write_all(b"[1,").unwrap();
        cancel.store(true, Ordering::Relaxed);
        // The next buffer that reaches the file fails (after at most one buffer of output).
        let e = (0..10_000)
            .find_map(|i| sink.write_all(format!("{i},").repeat(100).as_bytes()).err())
            .expect("a write fails once cancelled");
        assert!(matches!(sink.error(e), DimaError::Cancelled));
        drop(sink);
        assert!(entries(dir.path()).is_empty(), "{:?}", entries(dir.path()));
    }

    #[test]
    fn compressed_outputs_round_trip_from_small_writes() {
        let dir = tempfile::tempdir().unwrap();
        let text: Vec<u8> = (0..200_000u32)
            .flat_map(|i| format!("{i},").into_bytes())
            .collect();
        for (name, compression) in [
            ("a.gz", OutputCompression::Gzip),
            ("a.zst", OutputCompression::Zstd),
        ] {
            let dest = dir.path().join(name);
            let mut sink = Sink::create(&Target::Path(dest.clone()), compression).unwrap();
            // Serialisers write small pieces.
            for chunk in text.chunks(3) {
                sink.write_all(chunk).unwrap();
            }
            let written = sink.commit().unwrap();
            let stored = fs::read(&dest).unwrap();
            assert_eq!(written.bytes, stored.len() as u64);
            let mut decoded = Vec::new();
            match compression {
                OutputCompression::Gzip => {
                    flate2::read::GzDecoder::new(&stored[..])
                        .read_to_end(&mut decoded)
                        .unwrap();
                }
                _ => decoded = zstd::decode_all(&stored[..]).unwrap(),
            }
            assert_eq!(decoded, text, "{name}");
        }
    }

    #[test]
    fn long_names_get_short_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join(format!("{}.json", "r".repeat(245)));
        let mut sink = Sink::create(&Target::Path(dest.clone()), OutputCompression::None).unwrap();
        sink.write_all(b"{}").unwrap();
        sink.commit().unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"{}");
    }

    #[cfg(unix)]
    #[test]
    fn destinations() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("data.json");
        fs::write(&file, "x").unwrap();
        let link = dir.path().join("link.json");
        std::os::unix::fs::symlink("data.json", &link).unwrap();
        // A link is replaced through: its target is written atomically.
        assert_eq!(
            resolve_destination(&link),
            Destination::Atomic(file.clone())
        );
        let missing = dir.path().join("new.json");
        assert_eq!(
            resolve_destination(&missing),
            Destination::Atomic(missing.clone())
        );
        // Descriptor aliases are appended to in place, never canonicalised and replaced.
        for alias in ["/dev/stdout", "/dev/stderr", "/dev/fd/1", "/proc/self/fd/1"] {
            assert_eq!(
                resolve_destination(Path::new(alias)),
                Destination::Direct {
                    path: PathBuf::from(alias),
                    append: true
                },
                "{alias}"
            );
        }
        let to_stdout = dir.path().join("to_stdout");
        std::os::unix::fs::symlink("/dev/stdout", &to_stdout).unwrap();
        assert!(matches!(
            resolve_destination(&to_stdout),
            Destination::Direct { append: true, .. }
        ));
        // Devices are written in place.
        assert_eq!(
            resolve_destination(Path::new("/dev/null")),
            Destination::Direct {
                path: PathBuf::from("/dev/null"),
                append: false
            }
        );
        // What planners treat as a stream is exactly what is written in place.
        assert!(written_in_place(Path::new("/dev/null")));
        assert!(written_in_place(Path::new("/dev/stdout")));
        assert!(written_in_place(&to_stdout));
        assert!(!written_in_place(&file));
        assert!(!written_in_place(&link));
        assert!(!written_in_place(&missing));
        assert!(!written_in_place(dir.path()), "a folder is not a stream");
        let to_dir = dir.path().join("to_dir");
        std::os::unix::fs::symlink(dir.path(), &to_dir).unwrap();
        assert!(!written_in_place(&to_dir), "nor is a link to one");
    }

    /// A link loop and a name too long for any file system fail when the sink is created,
    /// before anything is computed, and nothing is left behind.
    #[cfg(unix)]
    #[test]
    fn unusable_names_fail_at_creation() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.json"), dir.path().join("b.json"));
        std::os::unix::fs::symlink("b.json", &a).unwrap();
        std::os::unix::fs::symlink("a.json", &b).unwrap();
        let Err(e) = Sink::create(&Target::Path(a.clone()), OutputCompression::None) else {
            panic!("a link loop is refused");
        };
        assert!(
            e.to_string().contains("too many levels of symbolic links") || {
                let cause = std::error::Error::source(&e).map(ToString::to_string);
                cause.is_some_and(|c| c.contains("too many levels"))
            },
            "{e:?}"
        );
        assert!(fs::symlink_metadata(&a).unwrap().file_type().is_symlink());
        let long = dir.path().join(format!("{}.json", "a".repeat(300)));
        let Err(e) = Sink::create(&Target::Path(long), OutputCompression::None) else {
            panic!("a name too long is refused");
        };
        assert!(format!("{e:?}").contains("file name too long"), "{e:?}");
        assert_eq!(entries(dir.path()).len(), 2, "only the two links");
        // The longest name that fits is written.
        let fits = dir.path().join(format!("{}.json", "a".repeat(250)));
        let mut sink = Sink::create(&Target::Path(fits.clone()), OutputCompression::None).unwrap();
        sink.write_all(b"{}").unwrap();
        sink.commit().unwrap();
        assert_eq!(fs::read(&fits).unwrap(), b"{}");
    }

    #[test]
    fn commit_replaces_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("out.json");
        fs::write(&p, "old").unwrap();
        let mut s = Sink::create(&Target::Path(p.clone()), OutputCompression::None).unwrap();
        s.write_all(b"new").unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            "old",
            "unchanged until commit"
        );
        assert_eq!(
            entries(dir.path()).len(),
            2,
            "temporary file beside the target"
        );
        s.commit().unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "new");
        assert_eq!(entries(dir.path()), ["out.json"]);
    }

    #[test]
    fn uncommitted_sink_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("never.json");
        {
            let mut s = Sink::create(&Target::Path(p.clone()), OutputCompression::Gzip).unwrap();
            s.write_all(b"partial").unwrap();
        }
        assert!(entries(dir.path()).is_empty());
    }

    #[test]
    fn compressed_outputs_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        for (c, name) in [
            (OutputCompression::Gzip, "a.gz"),
            (OutputCompression::Zstd, "a.zst"),
        ] {
            let p = dir.path().join(name);
            let mut s = Sink::create(&Target::Path(p.clone()), c).unwrap();
            s.write_all(b"hello\n").unwrap();
            s.commit().unwrap();
            let bytes = fs::read(&p).unwrap();
            let mut out = String::new();
            match c {
                OutputCompression::Gzip => {
                    flate2::read::MultiGzDecoder::new(&bytes[..])
                        .read_to_string(&mut out)
                        .unwrap();
                }
                _ => {
                    out = String::from_utf8(zstd::decode_all(&bytes[..]).unwrap()).unwrap();
                }
            }
            assert_eq!(out, "hello\n");
        }
    }

    #[test]
    fn missing_directory_is_an_io_error() {
        let e = Sink::create(
            &Target::Path("/nonexistent-dir/x.json".into()),
            OutputCompression::None,
        )
        .err()
        .unwrap();
        assert_eq!(e.class(), crate::error::ExitClass::Io);
        assert!(e.to_string().contains("/nonexistent-dir/x.json"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_followed_and_permissions_respect_umask() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.json");
        fs::write(&real, "old").unwrap();
        let link = dir.path().join("link.json");
        symlink(&real, &link).unwrap();
        let mut s = Sink::create(&Target::Path(link.clone()), OutputCompression::None).unwrap();
        s.write_all(b"new").unwrap();
        s.commit().unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        let mode = fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_ne!(mode, 0o600, "not tempfile's private mode");
        assert_eq!(mode & 0o200, 0o200);
        // A replaced file keeps its permissions.
        fs::set_permissions(&real, fs::Permissions::from_mode(0o640)).unwrap();
        let mut s = Sink::create(&Target::Path(link), OutputCompression::None).unwrap();
        s.write_all(b"newer").unwrap();
        s.commit().unwrap();
        assert_eq!(fs::read_to_string(&real).unwrap(), "newer");
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn fifos_are_written_directly() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("fifo");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&p)
                .status()
                .unwrap()
                .success()
        );
        let reader_path = p.clone();
        let t = std::thread::spawn(move || fs::read_to_string(reader_path).unwrap());
        let mut s = Sink::create(&Target::Path(p), OutputCompression::None).unwrap();
        s.write_all(b"through the pipe").unwrap();
        s.commit().unwrap();
        assert_eq!(t.join().unwrap(), "through the pipe");
        assert_eq!(entries(dir.path()), ["fifo"]);
    }
}
