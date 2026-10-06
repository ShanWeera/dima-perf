//! The library's error type and its mapping to process exit classes.
//!
//! Every error says *what* failed and *where* (source, record, line, column, byte), and most
//! carry a hint telling the user how to fix it. Front ends render the error with
//! [`DimaError::report`] (GUI) or line by line (`error:` / `caused by:` / `hint:`, CLI), and map
//! [`DimaError::class`] to the documented exit codes.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Convenience alias used throughout the library.
pub type Result<T, E = DimaError> = std::result::Result<T, E>;

/// Exit classes of the command-line interface (documented exit codes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExitClass {
    /// Exit 1: a bug in DiMA.
    Internal,
    /// Exit 2: invalid command line or parameters.
    Usage,
    /// Exit 3: a file or stream could not be opened, read or written, or the operating system
    /// refused a resource (a thread).
    Io,
    /// Exit 4: the input data are malformed or unsupported.
    InvalidInput,
    /// Exit 130 (or 143/129, chosen by the front end from the signal received).
    Cancelled,
}

impl ExitClass {
    /// The process exit code (cancellation defaults to 130, the SIGINT convention).
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Internal => 1,
            Self::Usage => 2,
            Self::Io => 3,
            Self::InvalidInput => 4,
            Self::Cancelled => 130,
        }
    }
}

/// A position in an input stream. Every field except `source` is optional because not every
/// error has every coordinate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Location {
    /// Display name of the input: its file name, or `standard input`.
    pub source: String,
    /// 1-based FASTA record number.
    pub record: Option<u64>,
    /// 1-based line number in the decompressed text.
    pub line: Option<u64>,
    /// 1-based alignment column (residue position, whitespace excluded).
    pub column: Option<u64>,
    /// 0-based byte offset in the decompressed text.
    pub byte: Option<u64>,
}

impl Location {
    /// A location naming only the source.
    #[must_use]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            ..Self::default()
        }
    }

    /// Sets the record number.
    #[must_use]
    pub fn record(mut self, record: u64) -> Self {
        self.record = Some(record);
        self
    }

    /// Sets the line number.
    #[must_use]
    pub fn line(mut self, line: u64) -> Self {
        self.line = Some(line);
        self
    }

    /// Sets the column.
    #[must_use]
    pub fn column(mut self, column: u64) -> Self {
        self.column = Some(column);
        self
    }

    /// Sets the byte offset.
    #[must_use]
    pub fn byte(mut self, byte: u64) -> Self {
        self.byte = Some(byte);
        self
    }
}

/// A file name as shown in messages: control characters (a newline in a name would split the
/// message) are written as escapes such as `\u{a}`.
fn shown_name(name: &str) -> std::borrow::Cow<'_, str> {
    if !name.chars().any(char::is_control) {
        return name.into();
    }
    let mut out = String::with_capacity(name.len() + 8);
    for c in name.chars() {
        if c.is_control() {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    out.into()
}

impl fmt::Display for Location {
    /// `input.fasta: record 12 (line 45, column 7, byte 1234)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&shown_name(&self.source))?;
        let mut details = Vec::with_capacity(3);
        if let Some(l) = self.line {
            details.push(format!("line {l}"));
        }
        if let Some(c) = self.column {
            details.push(format!("column {c}"));
        }
        if let Some(b) = self.byte {
            details.push(format!("byte {b}"));
        }
        match (self.record, details.is_empty()) {
            (Some(r), true) => write!(f, ": record {r}"),
            (Some(r), false) => write!(f, ": record {r} ({})", details.join(", ")),
            (None, false) => write!(f, ": {}", details.join(", ")),
            (None, true) => Ok(()),
        }
    }
}

/// What an I/O operation was doing when it failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IoAction {
    /// Opening an existing file.
    Open,
    /// Reading.
    Read,
    /// Creating a new file (including temporary files).
    Create,
    /// Writing.
    Write,
    /// Flushing data to stable storage.
    Sync,
    /// Atomically replacing the destination.
    Replace,
    /// Removing a file.
    Remove,
    /// Creating a directory.
    CreateDir,
    /// Reading file metadata.
    Inspect,
}

impl IoAction {
    const fn verb(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Read => "read",
            Self::Create => "create",
            Self::Write => "write",
            Self::Sync => "flush",
            Self::Replace => "replace",
            Self::Remove => "remove",
            Self::CreateDir => "create directory",
            Self::Inspect => "inspect",
        }
    }
}

/// The file or stream an I/O operation used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IoTarget {
    /// A path on the file system.
    Path(PathBuf),
    /// Standard input.
    Stdin,
    /// Standard output.
    Stdout,
}

impl fmt::Display for IoTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(p) => write!(f, "'{}'", shown_name(&p.display().to_string())),
            Self::Stdin => f.write_str("standard input"),
            Self::Stdout => f.write_str("standard output"),
        }
    }
}

/// What to do when the system refuses threads, for this platform: the hint of
/// [`DimaError::System`] and of the front ends' warning when a run continues with fewer threads
/// than requested (D110).
pub const THREAD_LIMIT_HINT: &str = if cfg!(target_os = "macos") {
    // macOS limits the threads of each process (kern.num_taskthreads); `ulimit -u` counts
    // processes only.
    "use fewer threads; macOS limits the number of threads of each process"
} else if cfg!(windows) {
    "use fewer threads, or close other programs: every thread needs memory"
} else {
    "use fewer threads, or raise the system's limit on processes and threads (`ulimit -u`, or \
     the container's pids limit)"
};

/// Every error the library reports.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DimaError {
    /// Invalid parameters (exit 2).
    #[error("{message}")]
    Usage {
        /// What is wrong.
        message: String,
        /// How to fix it.
        hint: Option<String>,
    },
    /// The input is not a well-formed FASTA alignment (exit 4). Raised in every validation
    /// mode.
    #[error("{location}: {message}")]
    InputFormat {
        /// Where the problem is (boxed to keep `Result`s small).
        location: Box<Location>,
        /// What is wrong.
        message: String,
        /// How to fix it.
        hint: Option<String>,
    },
    /// The input is well-formed but its content is invalid or unsupported (exit 4).
    #[error("{}{message}", location.as_ref().map(|l| format!("{l}: ")).unwrap_or_default())]
    InputData {
        /// Where the problem is, when it has a single location.
        location: Option<Box<Location>>,
        /// What is wrong.
        message: String,
        /// How to fix it.
        hint: Option<String>,
    },
    /// A file or stream could not be opened, read or written (exit 3).
    #[error("cannot {} {target}", action.verb())]
    Io {
        /// The failed operation.
        action: IoAction,
        /// The file or stream.
        target: IoTarget,
        /// The operating-system error.
        #[source]
        source: io::Error,
    },
    /// The operating system refused a resource the run needs, such as a thread (exit 3): the
    /// environment's limits, not the input and not a bug.
    #[error("cannot start {what}")]
    System {
        /// What could not be started, e.g. `8 worker threads`.
        what: String,
        /// The operating-system error.
        #[source]
        source: io::Error,
    },
    /// The input's first batch differs from what a pre-check saw (D108): the file changed
    /// after it was checked (exit 3; only front ends that pre-check, such as the desktop app,
    /// set the expectation).
    #[error("'{source_name}' changed after it was checked")]
    InputChanged {
        /// Display name of the input.
        source_name: String,
    },
    /// The operation was cancelled by the user or a signal.
    #[error("cancelled")]
    Cancelled,
    /// A 64-bit counter would overflow (exit 4).
    #[error("{what} exceeds the 64-bit counter range")]
    Overflow {
        /// The quantity that overflowed.
        what: &'static str,
    },
    /// A broken internal invariant: a bug (exit 1).
    #[error("internal error: {message}")]
    Internal {
        /// The invariant that failed.
        message: String,
    },
}

impl DimaError {
    /// A usage error without a hint.
    #[must_use]
    pub fn usage(message: impl Into<String>) -> Self {
        Self::Usage {
            message: message.into(),
            hint: None,
        }
    }

    /// A structural input error at `location`.
    #[must_use]
    pub fn input_format(location: Location, message: impl Into<String>) -> Self {
        Self::InputFormat {
            location: Box::new(location),
            message: message.into(),
            hint: None,
        }
    }

    /// An invalid-data error, optionally at `location`.
    #[must_use]
    pub fn input_data(location: Option<Location>, message: impl Into<String>) -> Self {
        Self::InputData {
            location: location.map(Box::new),
            message: message.into(),
            hint: None,
        }
    }

    /// An I/O error.
    #[must_use]
    pub fn io(action: IoAction, target: IoTarget, source: io::Error) -> Self {
        Self::Io {
            action,
            target,
            source,
        }
    }

    /// A thread the run needs could not be started.
    #[must_use]
    pub fn system(what: impl Into<String>, source: io::Error) -> Self {
        Self::System {
            what: what.into(),
            source,
        }
    }

    /// An internal error (a bug).
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Names the input of an invalid-data error that has no location yet.
    #[must_use]
    pub(crate) fn in_source(mut self, name: &str) -> Self {
        if let Self::InputData {
            location: location @ None,
            ..
        } = &mut self
        {
            *location = Some(Box::new(Location::new(name.to_owned())));
        }
        self
    }

    /// Attaches a hint to errors that carry one; other errors are returned unchanged.
    #[must_use]
    pub fn with_hint(mut self, new_hint: impl Into<String>) -> Self {
        match &mut self {
            Self::Usage { hint, .. }
            | Self::InputFormat { hint, .. }
            | Self::InputData { hint, .. } => *hint = Some(new_hint.into()),
            Self::Io { .. }
            | Self::System { .. }
            | Self::InputChanged { .. }
            | Self::Cancelled
            | Self::Overflow { .. }
            | Self::Internal { .. } => {}
        }
        self
    }

    /// How to fix the error, if known.
    #[must_use]
    pub fn hint(&self) -> Option<&str> {
        match self {
            Self::Usage { hint, .. }
            | Self::InputFormat { hint, .. }
            | Self::InputData { hint, .. } => hint.as_deref(),
            Self::Io { source, .. } => match source.kind() {
                io::ErrorKind::NotFound => Some("check the path and the current directory"),
                io::ErrorKind::PermissionDenied => Some(
                    "check the permissions of the file and its directory (an output is written \
                     to a temporary file in its directory first, so the directory must be \
                     writable)",
                ),
                io::ErrorKind::StorageFull => Some("free some disk space and try again"),
                io::ErrorKind::QuotaExceeded => {
                    Some("the disk quota is used up; free some space or write elsewhere")
                }
                io::ErrorKind::FileTooLarge => Some(
                    "a file-size limit was reached (see `ulimit -f`); raise it or compress the output",
                ),
                io::ErrorKind::ReadOnlyFilesystem => {
                    Some("the file system is read-only; write the output elsewhere")
                }
                _ => None,
            },
            Self::InputChanged { .. } => {
                Some("check the file again, review the settings and start again")
            }
            Self::System { .. } => Some(THREAD_LIMIT_HINT),
            Self::Overflow { .. } => {
                Some("split the input into smaller alignments and analyse them separately")
            }
            Self::Internal { .. } => Some(concat!(
                "this is a bug in DiMA; please report it at ",
                env!("CARGO_PKG_REPOSITORY"),
                "/issues with the command line and, if possible, the input"
            )),
            Self::Cancelled => None,
        }
    }

    /// The exit class of the error.
    #[must_use]
    pub const fn class(&self) -> ExitClass {
        match self {
            Self::Usage { .. } => ExitClass::Usage,
            Self::InputFormat { .. } | Self::InputData { .. } | Self::Overflow { .. } => {
                ExitClass::InvalidInput
            }
            Self::Io { .. } | Self::System { .. } | Self::InputChanged { .. } => ExitClass::Io,
            Self::Cancelled => ExitClass::Cancelled,
            Self::Internal { .. } => ExitClass::Internal,
        }
    }

    /// `true` for a write to standard output that failed because the reader went away
    /// (`dima … | head`); front ends treat this as success.
    #[must_use]
    pub fn is_broken_pipe(&self) -> bool {
        let stdout = |t: &IoTarget| match t {
            IoTarget::Stdout => true,
            // `-o /dev/stdout` and its aliases are standard output too.
            IoTarget::Path(p) => {
                cfg!(unix)
                    && ["/dev/stdout", "/dev/fd/1", "/proc/self/fd/1"]
                        .iter()
                        .any(|alias| p == std::path::Path::new(alias))
            }
            IoTarget::Stdin => false,
        };
        matches!(
            self,
            Self::Io { target, source, .. }
                if stdout(target) && source.kind() == io::ErrorKind::BrokenPipe
        )
    }

    /// The full report as plain text: the message, every `caused by:` line and the hint, one
    /// per line (used by the GUI and by tests).
    #[must_use]
    pub fn report(&self) -> String {
        let mut out = self.to_string();
        let mut cause = std::error::Error::source(self);
        while let Some(c) = cause {
            out.push_str("\ncaused by: ");
            out.push_str(&c.to_string());
            cause = c.source();
        }
        if let Some(h) = self.hint() {
            out.push_str("\nhint: ");
            out.push_str(h);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_stays_small() {
        assert!(
            std::mem::size_of::<DimaError>() <= 64,
            "{}",
            std::mem::size_of::<DimaError>()
        );
    }

    #[test]
    fn refused_threads_are_not_bugs() {
        let e = DimaError::system(
            "8 worker threads",
            io::Error::from_raw_os_error(11), // EAGAIN on Linux
        );
        assert_eq!(e.to_string(), "cannot start 8 worker threads");
        assert_eq!(e.class(), ExitClass::Io);
        assert!(e.hint().unwrap().contains("fewer threads"));
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn control_characters_in_names_are_escaped() {
        let l = Location::new("bad\nname.fa").line(3);
        assert_eq!(l.to_string(), "bad\\u{a}name.fa: line 3");
        let t = IoTarget::Path("a\rb.fa".into());
        assert_eq!(t.to_string(), "'a\\u{d}b.fa'");
        assert_eq!(shown_name("Zürich spike.fa"), "Zürich spike.fa");
    }

    #[test]
    fn location_display() {
        let l = Location::new("a.fasta")
            .record(12)
            .line(45)
            .column(7)
            .byte(1234);
        assert_eq!(
            l.to_string(),
            "a.fasta: record 12 (line 45, column 7, byte 1234)"
        );
        assert_eq!(
            Location::new("a.fasta").record(3).to_string(),
            "a.fasta: record 3"
        );
        assert_eq!(
            Location::new("standard input").line(1).byte(0).to_string(),
            "standard input: line 1, byte 0"
        );
        assert_eq!(Location::new("x").to_string(), "x");
    }

    #[test]
    fn classes_and_codes() {
        let io = DimaError::io(
            IoAction::Open,
            IoTarget::Path("missing.fa".into()),
            io::Error::from(io::ErrorKind::NotFound),
        );
        assert_eq!(io.class(), ExitClass::Io);
        assert_eq!(io.class().code(), 3);
        assert_eq!(io.to_string(), "cannot open 'missing.fa'");
        assert_eq!(io.hint(), Some("check the path and the current directory"));
        assert!(
            io.report()
                .starts_with("cannot open 'missing.fa'\ncaused by: ")
        );
        assert_eq!(DimaError::usage("bad").class().code(), 2);
        assert_eq!(
            DimaError::input_format(Location::new("a"), "x")
                .class()
                .code(),
            4
        );
        assert_eq!(DimaError::input_data(None, "x").class().code(), 4);
        assert_eq!(DimaError::Overflow { what: "support" }.class().code(), 4);
        assert_eq!(DimaError::internal("x").class().code(), 1);
        assert_eq!(DimaError::Cancelled.class().code(), 130);
    }

    #[test]
    fn hints_and_reports() {
        let e = DimaError::input_format(Location::new("a.fa").record(2).line(3), "unexpected NUL")
            .with_hint("convert the file to plain text");
        assert_eq!(e.to_string(), "a.fa: record 2 (line 3): unexpected NUL");
        assert_eq!(
            e.report(),
            "a.fa: record 2 (line 3): unexpected NUL\nhint: convert the file to plain text"
        );
        let d = DimaError::input_data(None, "no positions");
        assert_eq!(d.to_string(), "no positions");
        // Hints are ignored by variants that compute their own.
        assert!(DimaError::Cancelled.with_hint("x").hint().is_none());
    }

    #[test]
    fn broken_pipe_detection() {
        let e = DimaError::io(
            IoAction::Write,
            IoTarget::Stdout,
            io::Error::from(io::ErrorKind::BrokenPipe),
        );
        assert!(e.is_broken_pipe());
        assert_eq!(e.to_string(), "cannot write standard output");
        let f = DimaError::io(
            IoAction::Write,
            IoTarget::Path("o".into()),
            io::Error::from(io::ErrorKind::BrokenPipe),
        );
        assert!(!f.is_broken_pipe());
    }
}
