//! Every file a command writes, and the check that no two of them, and no output and the input,
//! are the same file (D28, D103).
//!
//! [`Destinations::of`] lists the destinations from the options alone, before anything is read,
//! checked or run, so the run report — written even when a run fails — can be checked against
//! all of them first ([`check_run_report`]): a report that would replace a table, the input or
//! another output is refused before it could be written. [`check`] then compares every pair.
//!
//! Two destinations are the same file when their names lead to it: spelled
//! differently (`./x`, `d/../x`) or through symbolic links (dangling ones included). Names that
//! differ only in letter case, or in the Unicode form of an accented letter, are one file on
//! macOS (and, for letter case, on Windows): they are refused unless the file system shows
//! that they are two files — only one of them exists, or (on Unix) both exist with different
//! inodes. Hard links are not refused: an output replaces its own name only, so the other name
//! keeps the old file. Standard input and output have no name: on Unix they are compared by
//! the file behind them (its device and inode), so `dima analyze < x.fa -o x.fa` and `-o r.json
//! --hcs-output - > r.json` are refused too (and so is a hard link of the input given to
//! standard input). Two outputs that lead to one pipe, FIFO or socket (`-o /dev/fd/3
//! --hcs-output /dev/fd/3`, or a link to `/dev/stdout` next to `-`) would be mixed in it and
//! are refused. Devices such as `/dev/null` and terminals can take any number of outputs, and
//! appending standard output to a log (`>> log`) is never a clash.

use std::path::{Component, Path, PathBuf};

use dima_core::output::{self, Table, Target};
use dima_core::{DimaError, Result};

use super::args::OutputArgs;

/// What a destination receives, before its format is settled from its name. The run report's
/// `outputs[].role` uses [`super::outputs::Content::role`] instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The results (`-o`, or standard output when no output is named): JSON, JSONL or a table.
    Results,
    /// One of the four tables `--tsv-dir` writes.
    Table(Table),
    /// The conserved sequences (`--hcs-output`, or `-o` of `dima hcs`).
    Hcs,
    /// The run report (`--run-report`).
    RunReport,
    /// The citation `dima cite` prints on standard output.
    Citation,
}

impl Kind {
    /// The kind in words, for messages.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Results => "the results",
            Self::Table(Table::Positions) => "the positions table",
            Self::Table(Table::Variants) => "the variants table",
            Self::Table(Table::Metadata) => "the metadata table",
            Self::Table(Table::Hcs) => "the hcs table",
            Self::Hcs => "the conserved sequences",
            Self::RunReport => "the run report",
            Self::Citation => "the citation",
        }
    }
}

/// One file or stream a command writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destination {
    /// What it receives.
    pub kind: Kind,
    /// The option that names it; `None` for the results, which go to standard output when no
    /// output is named.
    pub option: Option<&'static str>,
    /// Where it goes.
    pub target: Target,
}

impl Destination {
    /// The destination in words, with the option that names it: `the positions table
    /// (--tsv-dir)`.
    #[must_use]
    pub fn describe(&self) -> String {
        match (self.option, self.kind) {
            (Some(option), _) => format!("{} ({option})", self.kind.describe()),
            // `dima cite` has no other destination.
            (None, Kind::Citation) => self.kind.describe().to_owned(),
            (None, _) => format!("{} (standard output by default)", self.kind.describe()),
        }
    }
}

/// Every file and folder a command writes, derived from its options alone.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Destinations {
    /// The files and streams, in writing order; the run report, when asked for, is last.
    pub files: Vec<Destination>,
    /// The folders that hold some of them (`--tsv-dir`), created when the outputs are opened.
    pub folders: Vec<PathBuf>,
}

impl Destinations {
    /// The destinations of `dima analyze` and `dima view`: the results (`-o`, or standard
    /// output when neither `-o`, `--tsv-dir` nor `--hcs-output` is given), the four tables of
    /// `--tsv-dir` named after `tables_base` (left out while it is unknown: `dima view` reads it
    /// from the results), the conserved sequences, then the run report.
    #[must_use]
    pub fn of(
        args: &OutputArgs,
        hcs_output: Option<&Path>,
        run_report: Option<&Path>,
        tables_base: Option<&str>,
    ) -> Self {
        let mut files = Vec::new();
        let named = args.output.is_some() || args.tsv_dir.is_some() || hcs_output.is_some();
        if let Some(path) = &args.output {
            files.push(Destination {
                kind: Kind::Results,
                option: Some("-o"),
                target: Target::from_arg(path),
            });
        } else if !named {
            files.push(Destination {
                kind: Kind::Results,
                option: None,
                target: Target::Stdout,
            });
        }
        let mut folders = Vec::new();
        if let Some(dir) = &args.tsv_dir {
            folders.push(dir.clone());
            if let Some(base) = tables_base {
                let base = output::file_stem(base);
                files.extend(Table::ALL.into_iter().map(|t| Destination {
                    kind: Kind::Table(t),
                    option: Some("--tsv-dir"),
                    target: Target::Path(dir.join(format!("{base}.{}.tsv", t.name()))),
                }));
            }
        }
        if let Some(path) = hcs_output {
            files.push(Destination {
                kind: Kind::Hcs,
                option: Some("--hcs-output"),
                target: Target::from_arg(path),
            });
        }
        if let Some(path) = run_report {
            files.push(Destination {
                kind: Kind::RunReport,
                option: Some("--run-report"),
                target: Target::from_arg(path),
            });
        }
        Self { files, folders }
    }

    /// The destination of `dima hcs`: the conserved sequences, on `-o` or standard output.
    #[must_use]
    pub fn of_hcs(output: Option<&Path>) -> Self {
        Self {
            files: vec![Destination {
                kind: Kind::Hcs,
                option: output.map(|_| "-o"),
                target: output.map_or(Target::Stdout, Target::from_arg),
            }],
            folders: Vec::new(),
        }
    }

    /// The destination of `dima cite`: the citation, on standard output.
    #[must_use]
    pub fn of_cite() -> Self {
        Self {
            files: vec![Destination {
                kind: Kind::Citation,
                option: None,
                target: Target::Stdout,
            }],
            folders: Vec::new(),
        }
    }

    /// The run report, if one is asked for.
    #[must_use]
    pub fn run_report(&self) -> Option<&Destination> {
        self.files.iter().find(|d| d.kind == Kind::RunReport)
    }

    /// The `--tsv-dir` folder whose creation also creates the folder `file` is written in
    /// (that folder itself, or one inside it), when that folder does not exist yet.
    #[must_use]
    pub fn folder_for(&self, file: &Path) -> Option<&Path> {
        let parent = file.parent().filter(|p| !p.as_os_str().is_empty())?;
        if parent.is_dir() {
            return None;
        }
        let parent = resolved(parent);
        self.folders
            .iter()
            .find(|f| resolved(f).starts_with(&parent))
            .map(PathBuf::as_path)
    }
}

/// The input a command reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input<'a> {
    /// A file named on the command line.
    Path(&'a Path),
    /// Standard input (`-`, or no input given).
    Stdin,
}

impl<'a> Input<'a> {
    /// The input of `-i`: standard input for `-`, or when it is not given.
    #[must_use]
    pub fn of(arg: Option<&'a Path>) -> Self {
        match arg {
            Some(p) if p.as_os_str() != "-" => Self::Path(p),
            _ => Self::Stdin,
        }
    }
}

/// Where the bytes of an entry go (or come from).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Place {
    /// Standard output: `-`, or a name of descriptor 1 such as `/dev/stdout`. Two outputs can
    /// never share it.
    Stdout,
    /// Another open descriptor (standard input, `/dev/fd/N`): only the file behind it, if any,
    /// tells what it is.
    Descriptor,
    /// A device such as `/dev/null`, written in place: any number of outputs may share one.
    Device,
    /// A file (or FIFO) reached by its name.
    Named {
        /// The file a write would reach: symbolic links followed as the writer follows them,
        /// the rest of the path resolved as far as it exists.
        exact: String,
        /// `exact` without letter case, in Unicode normalization form C: equal for two names
        /// that may be one file.
        key: String,
    },
}

/// What tells two entries apart: their place and, on Unix, the existing file behind them, by
/// device and inode.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    place: Place,
    inode: Option<(u64, u64)>,
    /// What is behind them is a pipe, a FIFO or a socket, not a regular file: two outputs
    /// written to it would be mixed.
    pipe: bool,
    /// What is behind them is a socket, which the input may share with an output.
    socket: bool,
}

impl Identity {
    /// The identity of an output.
    fn of_target(target: &Target) -> Self {
        match target {
            Target::Path(p) if !names_stdout(p) => Self::of_path(p),
            _ => Self::of_stream(Place::Stdout, Stream::Stdout),
        }
    }

    /// The identity of the input.
    fn of_input(input: Input<'_>) -> Self {
        match input {
            Input::Path(p) => Self::of_path(p),
            Input::Stdin => Self::of_stream(Place::Descriptor, Stream::Stdin),
        }
    }

    fn of_stream(place: Place, stream: Stream) -> Self {
        let id = stream_id(stream);
        Self {
            place,
            inode: id.map(|(inode, _)| inode),
            pipe: id.is_some_and(|(_, b)| b != Behind::File),
            socket: id.is_some_and(|(_, b)| b == Behind::Socket),
        }
    }

    fn of_path(path: &Path) -> Self {
        let meta = std::fs::metadata(path).ok();
        if meta.as_ref().is_some_and(is_device) || output::names_device(path) {
            return Self {
                place: Place::Device,
                inode: None,
                pipe: false,
                socket: false,
            };
        }
        let id = meta.as_ref().and_then(file_id);
        let inode = id.map(|(inode, _)| inode);
        let pipe = id.is_some_and(|(_, b)| b != Behind::File);
        let socket = id.is_some_and(|(_, b)| b == Behind::Socket);
        if dima_core::input::is_descriptor_alias(path) {
            return Self {
                place: Place::Descriptor,
                inode,
                pipe,
                socket,
            };
        }
        let exact = resolved(&output::destination_of(path))
            .to_string_lossy()
            .into_owned();
        let key = output::folded(Path::new(&exact));
        Self {
            place: Place::Named { exact, key },
            inode,
            pipe,
            socket,
        }
    }

    /// Standard output or another descriptor: compared with the others by inode only.
    const fn is_stream(&self) -> bool {
        matches!(self.place, Place::Stdout | Place::Descriptor)
    }
}

/// `/dev/stdout`, `/dev/fd/1` and `/proc/self/fd/1` are standard output (Unix).
#[must_use]
pub fn names_stdout(p: &Path) -> bool {
    cfg!(unix)
        && ["/dev/stdout", "/dev/fd/1", "/proc/self/fd/1"]
            .iter()
            .any(|alias| p == Path::new(alias))
}

/// A character or block device (on Windows: anything that is neither a file nor a folder).
fn is_device(m: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        let t = m.file_type();
        t.is_char_device() || t.is_block_device()
    }
    #[cfg(not(unix))]
    {
        !m.is_file() && !m.is_dir()
    }
}

/// What is behind a name or a descriptor, besides its device and inode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Behind {
    /// A regular file.
    File,
    /// A pipe or a FIFO: one way, so it cannot be both the input and an output.
    Pipe,
    /// A socket, which can be read and written (a service started by inetd).
    Socket,
}

/// The device and inode of what a write to `m` reaches (Unix), and what it is; `None` for
/// devices and folders.
fn file_id(m: &std::fs::Metadata) -> Option<((u64, u64), Behind)> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{FileTypeExt, MetadataExt};
        let behind = if m.is_file() {
            Behind::File
        } else if m.file_type().is_socket() {
            Behind::Socket
        } else {
            Behind::Pipe
        };
        (!is_device(m) && !m.is_dir()).then(|| ((m.dev(), m.ino()), behind))
    }
    #[cfg(not(unix))]
    {
        let _ = m;
        None
    }
}

#[derive(Clone, Copy)]
enum Stream {
    Stdin,
    Stdout,
}

/// The file behind standard input or output (Unix), as [`file_id`]: a regular file (`< x.fa`,
/// `> r.json`, `>> log`), or a pipe, FIFO or socket (`| wc`). Read from a duplicate of the
/// descriptor, so nothing is consumed.
fn stream_id(stream: Stream) -> Option<((u64, u64), Behind)> {
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;
        let fd = match stream {
            Stream::Stdin => std::io::stdin().as_fd().try_clone_to_owned(),
            Stream::Stdout => std::io::stdout().as_fd().try_clone_to_owned(),
        };
        let meta = std::fs::File::from(fd.ok()?).metadata().ok()?;
        file_id(&meta)
    }
    #[cfg(not(unix))]
    {
        let _ = stream;
        None
    }
}

/// `path` made absolute, with its deepest existing ancestor canonicalised and the rest
/// appended (`.` and `..` resolved by name): two spellings of a file in a folder that does not
/// exist yet (`newdir/x`, `./newdir/x`) resolve alike.
fn resolved(path: &Path) -> PathBuf {
    let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let parts: Vec<Component<'_>> = absolute.components().collect();
    for split in (0..=parts.len()).rev() {
        let head: PathBuf = parts[..split].iter().collect();
        if head.as_os_str().is_empty() {
            continue;
        }
        if let Ok(mut out) = std::fs::canonicalize(&head) {
            for c in &parts[split..] {
                match c {
                    Component::ParentDir => {
                        out.pop();
                    }
                    Component::CurDir => {}
                    other => out.push(other.as_os_str()),
                }
            }
            return out;
        }
    }
    absolute
}

/// The role of an entry of the check.
#[derive(Clone, Copy)]
enum Role<'a> {
    Input,
    Folder,
    Output(&'a Destination),
}

/// One thing the check compares: the input, the `--tsv-dir` folder or a destination.
struct Entry<'a> {
    role: Role<'a>,
    /// As the user wrote it, or `standard input`/`standard output`.
    shown: String,
    id: Identity,
}

impl Entry<'_> {
    /// The entry in words.
    fn what(&self) -> String {
        match self.role {
            Role::Input => "the input".to_owned(),
            Role::Folder => "the --tsv-dir folder".to_owned(),
            Role::Output(d) => d.describe(),
        }
    }

    const fn is_report(&self) -> bool {
        matches!(self.role, Role::Output(d) if matches!(d.kind, Kind::RunReport))
    }

    const fn is_default_results(&self) -> bool {
        matches!(self.role, Role::Output(d) if d.option.is_none() && matches!(d.kind, Kind::Results))
    }
}

fn entries<'a>(destinations: &'a Destinations, input: Input<'_>) -> Vec<Entry<'a>> {
    let mut out = vec![Entry {
        role: Role::Input,
        shown: match input {
            Input::Path(p) => p.display().to_string(),
            Input::Stdin => "standard input".to_owned(),
        },
        id: Identity::of_input(input),
    }];
    out.extend(destinations.folders.iter().map(|f| Entry {
        role: Role::Folder,
        shown: f.display().to_string(),
        id: Identity::of_path(f),
    }));
    out.extend(destinations.files.iter().map(|d| Entry {
        role: Role::Output(d),
        shown: match &d.target {
            Target::Stdout => "standard output".to_owned(),
            Target::Path(p) => p.display().to_string(),
        },
        id: Identity::of_target(&d.target),
    }));
    out
}

/// Refuses destinations that would replace each other or the input: the same file named twice
/// or spelled two ways, two outputs on standard output, and (on Unix) standard input or output
/// that is a file another output replaces.
///
/// # Errors
/// A usage error naming both and the fix.
pub fn check(destinations: &Destinations, input: Input<'_>) -> Result<()> {
    let all = entries(destinations, input);
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            if let Some(e) = clash(a, b) {
                return Err(e);
            }
        }
    }
    Ok(())
}

/// [`check`] for the run report alone: run before anything else, because the report is also
/// written when a run fails, even when it fails on another of these checks.
///
/// # Errors
/// A usage error naming the report, what it would replace and the fix.
pub fn check_run_report(destinations: &Destinations, input: Input<'_>) -> Result<()> {
    let all = entries(destinations, input);
    let Some(report) = all.iter().position(Entry::is_report) else {
        return Ok(());
    };
    for (i, other) in all.iter().enumerate() {
        if i != report {
            if let Some(e) = clash(&all[report], other) {
                return Err(e);
            }
        }
    }
    Ok(())
}

fn usage(message: impl Into<String>, hint: impl Into<String>) -> DimaError {
    DimaError::usage(message).with_hint(hint)
}

/// How to resolve a clash between `a` and `b`.
fn fix(a: &Entry<'_>, b: &Entry<'_>) -> String {
    // `dima cite` has no output option: its only output is standard output.
    if [a, b]
        .iter()
        .any(|e| matches!(e.role, Role::Output(d) if d.kind == Kind::Citation))
    {
        return "redirect standard output to another file; a --from file is never overwritten"
            .to_owned();
    }
    if [a, b].iter().any(|e| matches!(e.role, Role::Input)) {
        return "choose another output path; the input is never overwritten".to_owned();
    }
    if let Some(report) = [a, b].into_iter().find(|e| e.is_report()) {
        // A device or descriptor (`/dev/stdout`) is no folder to suggest a name in.
        let suggestion = match report.role {
            Role::Output(Destination {
                target: Target::Path(p),
                ..
            }) if !output::written_in_place(p) => report_suggestion(p),
            _ => "run-report.json".to_owned(),
        };
        return format!("give the run report its own file, such as '{suggestion}'");
    }
    "give every output its own file".to_owned()
}

/// A free name for a run report that clashed at `path`: `tables/x.positions.tsv` →
/// `tables/x.run-report.json`.
fn report_suggestion(path: &Path) -> String {
    let stem = path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.split('.').next())
        .filter(|s| !s.is_empty() && *s != "-")
        .unwrap_or("dima");
    let name = format!("{stem}.run-report.json");
    match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir.join(name).display().to_string(),
        _ => name,
    }
}

/// The error when `a` and `b` would be one file or stream, `None` when they are distinct.
fn clash(a: &Entry<'_>, b: &Entry<'_>) -> Option<DimaError> {
    if a.id.place == Place::Device || b.id.place == Place::Device {
        return None;
    }
    if a.id.place == Place::Stdout && b.id.place == Place::Stdout {
        let hint = if a.is_default_results() || b.is_default_results() {
            format!(
                "the results go to standard output when neither -o, --tsv-dir nor --hcs-output \
                 is given: write them to a file with -o FILE, or discard them with -o {} -O json",
                if cfg!(windows) { "NUL" } else { "/dev/null" }
            )
        } else {
            "send at most one output to '-' (standard output)".to_owned()
        };
        return Some(usage(
            format!(
                "two outputs would go to standard output: {} and {}",
                a.what(),
                b.what()
            ),
            hint,
        ));
    }
    if let (Place::Named { exact: ea, key: ka }, Place::Named { exact: eb, key: kb }) =
        (&a.id.place, &b.id.place)
    {
        if ka == kb {
            if ea == eb {
                return named_clash(a, b, true);
            }
            match output::one_file(Path::new(ea), Path::new(eb)) {
                Some(true) => return named_clash(a, b, true),
                None => return named_clash(a, b, false),
                Some(false) => {}
            }
        }
    }
    let same_file = a.id.inode.is_some() && a.id.inode == b.id.inode;
    if same_file {
        let input = matches!(a.role, Role::Input) || matches!(b.role, Role::Input);
        // Two outputs into one pipe would be mixed in it, and an output on the pipe or FIFO
        // the input comes from would keep the input from ever ending. Only a socket may be
        // both (a service started by inetd reads and writes one).
        if a.id.pipe && !(input && a.id.socket) {
            return Some(stream_clash(a, b));
        }
        if !a.id.pipe && (a.id.is_stream() || b.id.is_stream()) {
            return Some(stream_clash(a, b));
        }
    }
    None
}

/// Why the two named entries may be one file ([`output::why_one_file`]).
fn possibly_one_file(a: &Entry<'_>, b: &Entry<'_>) -> &'static str {
    let exact = |e: &Entry<'_>| match &e.id.place {
        Place::Named { exact, .. } => PathBuf::from(exact),
        _ => PathBuf::new(),
    };
    output::why_one_file(&exact(a), &exact(b))
}

/// Two names of one file: `certain` when they resolve alike, possible when only letter case
/// differs.
fn named_clash(a: &Entry<'_>, b: &Entry<'_>, certain: bool) -> Option<DimaError> {
    // The input and the --tsv-dir folder: the planner reports a folder that is a file.
    let (first, second) = match (a.role, b.role) {
        (Role::Input, Role::Folder) | (Role::Folder, Role::Input) => return None,
        (Role::Output(_), Role::Input | Role::Folder) => (b, a),
        _ => (a, b),
    };
    let (message, possible) = match first.role {
        Role::Input => (
            format!(
                "output '{}' ({}) is the input file",
                second.shown,
                second.what()
            ),
            format!(
                "output '{}' ({}) may be the same file as the input '{}'",
                second.shown,
                second.what(),
                first.shown
            ),
        ),
        Role::Folder => (
            format!(
                "'{}' is given both as the --tsv-dir folder and as {}",
                second.shown,
                second.what()
            ),
            format!(
                "{} '{}' may be the same file as the --tsv-dir folder '{}'",
                second.what(),
                second.shown,
                first.shown
            ),
        ),
        Role::Output(_) => (
            format!(
                "'{}' would be written twice: as {} and as {}",
                first.shown,
                first.what(),
                second.what()
            ),
            format!(
                "{} '{}' may be the same file as {} '{}'",
                first.what(),
                first.shown,
                second.what(),
                second.shown
            ),
        ),
    };
    let fix = fix(a, b);
    Some(if certain {
        usage(message, fix)
    } else {
        usage(possible, format!("{}; {fix}", possibly_one_file(a, b)))
    })
}

/// Standard input or output (or another descriptor) is the file another entry names, or two
/// outputs lead to one pipe.
fn stream_clash(a: &Entry<'_>, b: &Entry<'_>) -> DimaError {
    if !a.id.is_stream() && !b.id.is_stream() {
        // Two names (links to `/dev/stdout`, a FIFO's two links) of one pipe.
        return usage(
            format!(
                "'{}' and '{}' lead to one pipe: {} and {} would both be written to it",
                a.shown,
                b.shown,
                a.what(),
                b.what()
            ),
            fix(a, b),
        );
    }
    if a.shown == b.shown {
        // One descriptor named twice (`-o /dev/fd/3 --hcs-output /dev/fd/3`).
        return usage(
            format!(
                "{} would be written twice: as {} and as {}",
                a.shown,
                a.what(),
                b.what()
            ),
            fix(a, b),
        );
    }
    let (stream, other) = if a.id.is_stream() { (a, b) } else { (b, a) };
    let it = if a.id.pipe { "pipe" } else { "file" };
    let message = match (stream.role, other.role) {
        (Role::Input, _) => {
            if other.id.is_stream() {
                format!(
                    "{}, where {} would go, is the input {it} (read from {})",
                    other.shown,
                    other.what(),
                    stream.shown
                )
            } else {
                format!(
                    "output '{}' ({}) is the input {it} (read from {})",
                    other.shown,
                    other.what(),
                    stream.shown
                )
            }
        }
        (_, Role::Input) => format!(
            "{}, where {} would go, is the input {it} '{}'",
            stream.shown,
            stream.what(),
            other.shown
        ),
        (_, _) if other.id.is_stream() => format!(
            "{} and {} are the same {it}: {} and {} would both be written to it",
            stream.shown,
            other.shown,
            stream.what(),
            other.what()
        ),
        (_, _) => format!(
            "{}, where {} would go, is the {it} '{}', which is also written as {}",
            stream.shown,
            stream.what(),
            other.shown,
            other.what()
        ),
    };
    usage(message, fix(a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(output: Option<&str>, tsv_dir: Option<&str>) -> OutputArgs {
        OutputArgs {
            output: output.map(PathBuf::from),
            output_format: None,
            table: None,
            tsv_dir: tsv_dir.map(PathBuf::from),
            pretty: false,
        }
    }

    fn kinds(d: &Destinations) -> Vec<(Kind, Option<&'static str>)> {
        d.files.iter().map(|f| (f.kind, f.option)).collect()
    }

    #[test]
    fn destinations_list_every_output() {
        // Nothing named: the results go to standard output, with the run report after them.
        let d = Destinations::of(&args(None, None), None, Some(Path::new("r.json")), None);
        assert_eq!(
            kinds(&d),
            [
                (Kind::Results, None),
                (Kind::RunReport, Some("--run-report"))
            ]
        );
        assert_eq!(d.files[0].target, Target::Stdout);
        assert_eq!(
            d.run_report().unwrap().target,
            Target::Path("r.json".into())
        );
        // --tsv-dir: the folder, and the four tables named after the base.
        let d = Destinations::of(
            &args(None, Some("t")),
            Some(Path::new("h.json")),
            None,
            Some("MERS spike"),
        );
        assert_eq!(d.folders, [PathBuf::from("t")]);
        assert_eq!(
            d.files[0].target,
            Target::Path(Path::new("t").join("MERS_spike.positions.tsv"))
        );
        assert_eq!(d.files.len(), 5, "no results on standard output");
        assert_eq!(d.files[4].kind, Kind::Hcs);
        assert!(d.run_report().is_none());
        // The tables are left out while their base is unknown.
        let d = Destinations::of(&args(Some("x.json"), Some("t")), None, None, None);
        assert_eq!(kinds(&d), [(Kind::Results, Some("-o"))]);
        assert_eq!(d.folders.len(), 1);
        let d = Destinations::of_hcs(None);
        assert_eq!(d.files[0].target, Target::Stdout);
        assert_eq!(
            d.files[0].describe(),
            "the conserved sequences (standard output by default)",
            "as dima analyze says it"
        );
        let d = Destinations::of_hcs(Some(Path::new("h.json")));
        assert_eq!(d.files[0].describe(), "the conserved sequences (-o)");
        assert_eq!(Destinations::of_cite().files[0].describe(), "the citation");
    }

    #[test]
    fn kinds_in_words() {
        let d = Destination {
            kind: Kind::Table(Table::Positions),
            option: Some("--tsv-dir"),
            target: Target::Stdout,
        };
        assert_eq!(d.describe(), "the positions table (--tsv-dir)");
        let d = Destination {
            kind: Kind::Results,
            option: None,
            target: Target::Stdout,
        };
        assert_eq!(d.describe(), "the results (standard output by default)");
        assert_eq!(Kind::RunReport.describe(), "the run report");
        assert_eq!(Kind::Hcs.describe(), "the conserved sequences");
    }

    #[test]
    fn report_suggestions() {
        assert_eq!(
            report_suggestion(&Path::new("tables").join("x.positions.tsv")),
            Path::new("tables")
                .join("x.run-report.json")
                .display()
                .to_string()
        );
        assert_eq!(report_suggestion(Path::new("r.json")), "r.run-report.json");
        assert_eq!(
            report_suggestion(Path::new(".hidden")),
            "dima.run-report.json"
        );
    }

    fn message(r: Result<()>) -> String {
        let e = r.unwrap_err();
        format!("{e}\nhint: {}", e.hint().unwrap_or_default())
    }

    #[test]
    fn names_of_one_file_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.json");
        let input = dir.path().join("in.fasta");
        std::fs::write(&input, ">a\nACGT\n").unwrap();
        let s = |p: &Path| p.to_str().unwrap().to_owned();
        // Two outputs on one file, in words.
        let d = Destinations::of(&args(Some(&s(&a)), None), Some(&a), None, None);
        let m = message(check(&d, Input::Stdin));
        assert!(
            m.contains("would be written twice: as the results (-o) and as the conserved sequences (--hcs-output)"),
            "{m}"
        );
        assert!(m.contains("hint: give every output its own file"), "{m}");
        // An output on the input, spelled differently.
        let other = dir.path().join(".").join("in.fasta");
        let d = Destinations::of(&args(Some(&s(&other)), None), None, None, None);
        let m = message(check(&d, Input::Path(&input)));
        assert!(m.contains("(the results (-o)) is the input file"), "{m}");
        assert!(m.contains("the input is never overwritten"), "{m}");
        // Through a folder that does not exist yet.
        let new = dir.path().join("new");
        let d = Destinations::of(
            &args(
                Some(&s(&new.join("..").join("new").join("x.positions.tsv"))),
                Some(&s(&new)),
            ),
            None,
            None,
            Some("x"),
        );
        let m = message(check(&d, Input::Stdin));
        assert!(
            m.contains("as the results (-o) and as the positions table (--tsv-dir)"),
            "{m}"
        );
        // Names that differ only in letter case: possibly one file.
        let d = Destinations::of(
            &args(Some(&s(&a)), None),
            None,
            Some(&dir.path().join("A.JSON")),
            None,
        );
        let m = message(check(&d, Input::Stdin));
        assert!(
            m.contains("may be the same file as") || m.contains("would be written twice"),
            "{m}"
        );
        assert!(
            m.contains("give the run report its own file, such as"),
            "{m}"
        );
        // The folder itself as a file.
        let d = Destinations::of(&args(Some(&s(&new)), Some(&s(&new))), None, None, Some("x"));
        let m = message(check(&d, Input::Stdin));
        assert!(
            m.contains("is given both as the --tsv-dir folder and as the results (-o)"),
            "{m}"
        );
        // Distinct files are fine; so is the input as the folder (the planner reports it).
        let d = Destinations::of(&args(Some(&s(&a)), Some(&s(&new))), None, None, Some("x"));
        assert!(check(&d, Input::Path(&input)).is_ok());
    }

    #[test]
    fn standard_output_takes_one_output() {
        let d = Destinations::of(&args(Some("-"), None), Some(Path::new("-")), None, None);
        let m = message(check(&d, Input::Stdin));
        assert!(
            m.contains("two outputs would go to standard output: the results (-o) and the conserved sequences (--hcs-output)"),
            "{m}"
        );
        assert!(m.contains("send at most one output to '-'"), "{m}");
        // The run report next to the default results: the hint explains the default.
        let d = Destinations::of(&args(None, None), None, Some(Path::new("-")), None);
        let m = message(check_run_report(&d, Input::Stdin));
        assert!(
            m.contains("the results go to standard output when neither -o"),
            "{m}"
        );
        assert!(m.contains("-O json"), "{m}");
        #[cfg(unix)]
        {
            // /dev/stdout is standard output too.
            let d = Destinations::of(
                &args(Some("-"), None),
                Some(Path::new("/dev/stdout")),
                None,
                None,
            );
            assert!(message(check(&d, Input::Stdin)).contains("two outputs would go"));
            // Devices take any number of outputs.
            let d = Destinations::of(
                &args(Some("/dev/null"), None),
                Some(Path::new("/dev/null")),
                Some(Path::new("/dev/null")),
                None,
            );
            assert!(check(&d, Input::Stdin).is_ok());
        }
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_lead_to_their_target() {
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("target.json", dir.path().join("link.json")).unwrap();
        let link = dir.path().join("link.json");
        let target = dir.path().join("target.json");
        let d = Destinations::of(
            &args(Some(link.to_str().unwrap()), None),
            Some(&target),
            None,
            None,
        );
        assert!(message(check(&d, Input::Stdin)).contains("would be written twice"));
    }

    #[test]
    fn the_run_report_is_checked_alone() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("tables");
        let table = t.join("x.positions.tsv");
        // A clash of two other outputs is not the report's.
        let d = Destinations::of(
            &args(Some("-"), Some(t.to_str().unwrap())),
            Some(Path::new("-")),
            Some(&dir.path().join("r.json")),
            Some("x"),
        );
        assert!(check_run_report(&d, Input::Stdin).is_ok());
        assert!(check(&d, Input::Stdin).is_err());
        // The report over a table, named first in the message.
        let d = Destinations::of(
            &args(None, Some(t.to_str().unwrap())),
            None,
            Some(&table),
            Some("x"),
        );
        let m = message(check_run_report(&d, Input::Stdin));
        assert!(
            m.contains("would be written twice: as the run report (--run-report) and as the positions table (--tsv-dir)"),
            "{m}"
        );
        assert!(m.contains("x.run-report.json"), "{m}");
        // The report over the input.
        let input = dir.path().join("in.fa");
        std::fs::write(&input, ">a\nAC\n").unwrap();
        let d = Destinations::of(&args(None, None), None, Some(&input), None);
        let m = message(check_run_report(&d, Input::Path(&input)));
        assert!(
            m.contains("(the run report (--run-report)) is the input file"),
            "{m}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn descriptors_are_compared_by_the_file_behind_them() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("x.json");
        std::fs::write(&file, "{}").unwrap();
        let named = |p: &Path| Entry {
            role: Role::Input,
            shown: p.display().to_string(),
            id: Identity::of_path(p),
        };
        let descriptor = Entry {
            role: Role::Input,
            shown: "standard input".into(),
            id: Identity {
                place: Place::Descriptor,
                inode: Identity::of_path(&file).inode,
                pipe: false,
                socket: false,
            },
        };
        let output = Destination {
            kind: Kind::Results,
            option: Some("-o"),
            target: Target::Path(file.clone()),
        };
        let out = Entry {
            role: Role::Output(&output),
            ..named(&file)
        };
        let e = clash(&descriptor, &out).unwrap();
        assert!(
            e.to_string()
                .contains("is the input file (read from standard input)"),
            "{e}"
        );
        // A hard link is another name: not refused.
        let hard = dir.path().join("hard.json");
        std::fs::hard_link(&file, &hard).unwrap();
        assert!(clash(&named(&hard), &out).is_none());
        // Standard output on a file another output replaces.
        let stdout = Destination {
            kind: Kind::Hcs,
            option: Some("--hcs-output"),
            target: Target::Stdout,
        };
        let s = Entry {
            role: Role::Output(&stdout),
            shown: "standard output".into(),
            id: Identity {
                place: Place::Stdout,
                inode: Identity::of_path(&file).inode,
                pipe: false,
                socket: false,
            },
        };
        let e = clash(&out, &s).unwrap();
        assert!(
            e.to_string().contains(
                "standard output, where the conserved sequences (--hcs-output) would go, is the file"
            ),
            "{e}"
        );
        // A device is never a clash, even when it is behind standard output too.
        let null = Entry {
            role: Role::Output(&output),
            shown: "/dev/null".into(),
            id: Identity::of_path(Path::new("/dev/null")),
        };
        assert_eq!(null.id.place, Place::Device);
        assert!(clash(&null, &s).is_none());
    }

    /// Two outputs into one pipe (a FIFO here, as `| wc` or `>(…)` would be) are refused,
    /// whatever names lead to it; the input may share one with an output.
    #[cfg(unix)]
    #[test]
    fn two_outputs_into_one_pipe_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let fifo = dir.path().join("p");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let named = Identity::of_path(&fifo);
        assert!(named.pipe && named.inode.is_some(), "{named:?}");
        let on = |place: Place| Identity {
            place,
            inode: named.inode,
            pipe: true,
            socket: false,
        };
        let results = Destination {
            kind: Kind::Results,
            option: Some("-o"),
            target: Target::Path("/dev/fd/3".into()),
        };
        let hcs = Destination {
            kind: Kind::Hcs,
            option: Some("--hcs-output"),
            target: Target::Stdout,
        };
        let entry = |d: &'static str, role, id| Entry {
            role,
            shown: d.into(),
            id,
        };
        let a = entry("/dev/fd/3", Role::Output(&results), on(Place::Descriptor));
        let b = entry("standard output", Role::Output(&hcs), on(Place::Stdout));
        let m = clash(&a, &b).unwrap().to_string();
        assert!(
            m.contains("/dev/fd/3 and standard output are the same pipe"),
            "{m}"
        );
        // A link to the pipe next to it, both named: one pipe.
        let link = dir.path().join("so.json");
        std::os::unix::fs::symlink(&fifo, &link).unwrap();
        let c = Entry {
            role: Role::Output(&hcs),
            shown: "x".into(),
            id: Identity {
                place: Place::Named {
                    exact: "elsewhere".into(),
                    key: "elsewhere".into(),
                },
                ..Identity::of_path(&link)
            },
        };
        let d = Entry {
            role: Role::Output(&results),
            ..entry("y", Role::Input, named.clone())
        };
        let m = clash(&c, &d).unwrap().to_string();
        assert!(m.contains("lead to one pipe"), "{m}");
        // The input on the FIFO an output writes: refused, since the input would never end.
        let input = entry("standard input", Role::Input, on(Place::Descriptor));
        let m = clash(&input, &a).unwrap().to_string();
        assert!(
            m.contains("is the input pipe (read from standard input)"),
            "{m}"
        );
        // A socket may be both (a service started by inetd).
        let socket = Identity {
            socket: true,
            ..on(Place::Descriptor)
        };
        let input = entry("standard input", Role::Input, socket.clone());
        let out = entry(
            "standard output",
            Role::Output(&hcs),
            Identity {
                place: Place::Stdout,
                ..socket
            },
        );
        assert!(clash(&input, &out).is_none());
    }

    /// The Unicode form of an accented letter is part of the key, like letter case (the rule
    /// itself is `output::one_file`, tested in the library).
    #[test]
    fn names_that_differ_only_in_case() {
        let dir = tempfile::tempdir().unwrap();
        // The Unicode form of an accented letter is part of the key, like case.
        let nfc = Identity::of_path(&dir.path().join("caf\u{e9}.json"));
        let nfd = Identity::of_path(&dir.path().join("cafe\u{301}.json"));
        match (nfc.place, nfd.place) {
            (Place::Named { key: a, .. }, Place::Named { key: b, .. }) => assert_eq!(a, b),
            other => panic!("{other:?}"),
        }
    }
}
