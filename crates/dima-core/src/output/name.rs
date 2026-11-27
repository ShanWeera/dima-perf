//! What an output's file name says: its format and its compression (D10).
//!
//! The single reader of output file names, shared by the command line and the desktop app, so
//! both accept the same names and neither writes bytes that contradict the name. It is pure:
//! it looks at the name only, never at the file system (whether a name is a stream such as
//! `/dev/null` is the writer's question, [`super::destination_of`]).
//!
//! A name is read from its end, case-insensitively: an optional compression extension (`.gz`
//! or `.zst`), then an optional format extension (`.json`; `.jsonl` or `.ndjson`; `.tsv` or
//! `.tab`). Names that ask for something DiMA cannot write are refused with a [`NameError`]
//! instead of being written under a misleading name:
//!
//! * a compression DiMA does not write (`.bz2`, `.xz`, `.zip`, …), or a compressed archive
//!   (`.tgz`, …);
//! * two compressions (`.json.gz.zst`);
//! * a name ending in a path separator, which names a folder, not a file.

use std::path::Path;

use super::{OutputCompression, OutputFormat};

/// What a valid output file name says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Named {
    /// The format named by the extension, if any.
    pub format: Option<OutputFormat>,
    /// The compression named by the extension (`None` = plain text).
    pub compression: OutputCompression,
}

/// Why an output file name cannot be honoured.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameError {
    /// The name ends in a compression or archive extension that DiMA does not write.
    UnsupportedCompression {
        /// The extension as written, with its dot (for example `.bz2`).
        suffix: String,
        /// What it names, in words (for example `bzip2 compression`).
        what: &'static str,
        /// The extension DiMA writes for the same purpose, when there is one (`.zstd` → `.zst`).
        instead: Option<&'static str>,
    },
    /// The name asks for two compressions (for example `.json.gz.zst`).
    TwoCompressions {
        /// The inner compression extension, with its dot.
        first: String,
        /// The outer compression extension, with its dot.
        second: String,
    },
    /// The name ends in a path separator, or its last part is `.` or `..`, so it names a
    /// folder.
    Folder,
}

impl NameError {
    /// The message and the hint, for `path` as the user wrote it.
    #[must_use]
    pub fn describe(&self, path: &Path) -> (String, String) {
        let shown = path.display().to_string();
        match self {
            Self::UnsupportedCompression {
                suffix,
                what,
                instead,
            } => {
                let stem = without(&shown, suffix);
                let hint = match instead {
                    Some(ext) => format!("DiMA writes this as {ext}: name it {stem}{ext}"),
                    None if what.contains("archive") => {
                        "DiMA writes one file per output, compressed with gzip or Zstandard: \
                         name it like results.json.gz or results.tsv.zst"
                            .to_owned()
                    }
                    None => format!(
                        "DiMA compresses with gzip or Zstandard: name it {stem}.gz or {stem}.zst"
                    ),
                };
                (
                    format!("'{shown}' asks for {what}, which DiMA does not write"),
                    hint,
                )
            }
            Self::TwoCompressions { first, second } => {
                let stem = without(&without(&shown, second), first);
                (
                    format!(
                        "'{shown}' asks for two compressions ({first}, then {second}); DiMA \
                         applies one"
                    ),
                    format!("name it {stem}.gz or {stem}.zst"),
                )
            }
            Self::Folder => (
                format!("'{shown}' names a folder, not a file"),
                "give the name of a file, not of a folder".to_owned(),
            ),
        }
    }
}

/// Compression and archive extensions DiMA does not write, with what they name and, when DiMA
/// writes the same thing under another extension, that extension.
const UNSUPPORTED: &[(&str, &str, Option<&str>)] = &[
    (".bz2", "bzip2 compression", None),
    (".bgz", "BGZF compression", None),
    (".xz", "xz compression", None),
    (".lzma", "LZMA compression", None),
    (".lz", "lzip compression", None),
    (".lz4", "LZ4 compression", None),
    (".z", "compress (.Z) compression", None),
    (".zip", "a ZIP archive", None),
    (".7z", "a 7-Zip archive", None),
    (".br", "Brotli compression", None),
    (".sz", "Snappy compression", None),
    (
        ".zstd",
        "Zstandard compression under the name .zstd",
        Some(".zst"),
    ),
    (
        ".gzip",
        "gzip compression under the name .gzip",
        Some(".gz"),
    ),
    (".tar.gz", "a compressed tar archive", None),
    (".tar.zst", "a compressed tar archive", None),
    (".tar", "a tar archive", None),
    (".tgz", "a compressed tar archive", None),
    (".tbz2", "a compressed tar archive", None),
    (".txz", "a compressed tar archive", None),
];

/// The compression extensions DiMA writes.
const COMPRESSIONS: &[(&str, OutputCompression)] = &[
    (".gz", OutputCompression::Gzip),
    (".zst", OutputCompression::Zstd),
];

impl OutputFormat {
    /// The file extensions of the format, the usual one first.
    #[must_use]
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Json => &[".json"],
            Self::Jsonl => &[".jsonl", ".ndjson"],
            Self::Tsv => &[".tsv", ".tab"],
        }
    }
}

impl OutputCompression {
    /// The file extension of the compression (`""` for plain text).
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::None => "",
            Self::Gzip => ".gz",
            Self::Zstd => ".zst",
        }
    }
}

/// Reads what `path`'s file name says about the output written to it.
///
/// # Errors
/// A [`NameError`] when the name asks for a compression DiMA does not write, for two
/// compressions, or names a folder.
pub fn read_name(path: &Path) -> Result<Named, NameError> {
    let raw = path.as_os_str().to_string_lossy();
    if names_folder(&raw) {
        return Err(NameError::Folder);
    }
    // ASCII lower-casing keeps every byte position, so a suffix found in `lower` can be cut
    // from `name` exactly as the user wrote it.
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    if let Some(&(suffix, what, instead)) = UNSUPPORTED.iter().find(|(s, ..)| lower.ends_with(s)) {
        return Err(NameError::UnsupportedCompression {
            suffix: name[name.len() - suffix.len()..].to_owned(),
            what,
            instead,
        });
    }
    let (stem, compression) = COMPRESSIONS
        .iter()
        .find(|(s, _)| lower.ends_with(s))
        .map_or((lower.as_str(), OutputCompression::None), |&(s, c)| {
            (&lower[..lower.len() - s.len()], c)
        });
    if compression != OutputCompression::None {
        let inner = COMPRESSIONS
            .iter()
            .map(|(s, _)| *s)
            .chain(UNSUPPORTED.iter().map(|(s, ..)| *s))
            .find(|s| stem.ends_with(s));
        if let Some(inner) = inner {
            let outer_at = stem.len();
            let inner_at = outer_at - inner.len();
            return Err(NameError::TwoCompressions {
                first: name[inner_at..outer_at].to_owned(),
                second: name[outer_at..].to_owned(),
            });
        }
    }
    let format = [OutputFormat::Jsonl, OutputFormat::Json, OutputFormat::Tsv]
        .into_iter()
        .find(|f| f.extensions().iter().any(|e| stem.ends_with(e)));
    Ok(Named {
        format,
        compression,
    })
}

/// `path` as file systems that ignore letter case and Unicode form compare names (macOS's by
/// default for both, Windows' for case): in lower case and normalization form C. Two names
/// equal once folded may be one file; [`one_file`] tells when the file system can.
#[must_use]
pub fn folded(path: &Path) -> String {
    use unicode_normalization::UnicodeNormalization;
    path.to_string_lossy().to_lowercase().nfc().collect()
}

/// Whether two names that are equal once [`folded`] but differ as written are one file:
/// `Some(false)` when only one of them exists (a file system that ignores the difference would
/// find both), the comparison of their device and inode when both exist (Unix), `None` when
/// the file system cannot tell (neither exists yet, or both exist where inodes are not
/// available). Neither name's symbolic links are followed: resolve them first.
#[must_use]
pub fn one_file(a: &Path, b: &Path) -> Option<bool> {
    match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
        (Ok(x), Ok(y)) => same_inode(&x, &y),
        (Err(_), Err(_)) => None,
        _ => Some(false),
    }
}

/// Why two names equal once [`folded`] may be one file, for messages: they differ only in
/// letter case, or only in how their accented letters are encoded.
#[must_use]
pub fn why_one_file(a: &Path, b: &Path) -> &'static str {
    if a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase() {
        "names that differ only in letter case are one file on macOS and Windows"
    } else {
        "names that differ only in how their accented letters are encoded (Unicode form) are one \
         file on macOS"
    }
}

#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
fn same_inode(x: &std::fs::Metadata, y: &std::fs::Metadata) -> Option<bool> {
    use std::os::unix::fs::MetadataExt;
    Some((x.dev(), x.ino()) == (y.dev(), y.ino()))
}

#[cfg(not(unix))]
const fn same_inode(_: &std::fs::Metadata, _: &std::fs::Metadata) -> Option<bool> {
    None
}

/// `true` when `raw` can only name a folder: it ends in a path separator (other than a lone
/// `/`, which [`read_name`]'s callers refuse as an existing folder), or its last part is `.` or
/// `..` (`out.json/.`), which `Path::file_name` would skip.
fn names_folder(raw: &str) -> bool {
    if raw.len() > 1 && raw.ends_with(std::path::is_separator) {
        return true;
    }
    let last = raw
        .rsplit(std::path::is_separator)
        .next()
        .unwrap_or_default();
    last == "." || last == ".."
}

/// `shown` without its ending `suffix` (compared case-insensitively), for a hint; when `shown`
/// does not end with it (a path such as `x.bz2/.`), the bare file name without it.
fn without(shown: &str, suffix: &str) -> String {
    let lower = shown.to_ascii_lowercase();
    if lower.ends_with(&suffix.to_ascii_lowercase()) {
        shown[..shown.len() - suffix.len()].to_owned()
    } else {
        "results".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{NameError, Named, OutputCompression as C, OutputFormat as F, read_name};
    use std::path::Path;

    fn ok(s: &str) -> Named {
        read_name(Path::new(s)).unwrap_or_else(|e| panic!("{s}: {e:?}"))
    }

    fn err(s: &str) -> NameError {
        read_name(Path::new(s)).expect_err(s)
    }

    #[test]
    fn recognised_names() {
        let n = |format, compression| Named {
            format,
            compression,
        };
        assert_eq!(ok("out.json"), n(Some(F::Json), C::None));
        assert_eq!(ok("dir.v1/OUT.JSON.GZ"), n(Some(F::Json), C::Gzip));
        assert_eq!(ok("x.jsonl.zst"), n(Some(F::Jsonl), C::Zstd));
        assert_eq!(ok("x.ndjson"), n(Some(F::Jsonl), C::None));
        assert_eq!(ok("x.NDJSON.gz"), n(Some(F::Jsonl), C::Gzip));
        assert_eq!(ok("x.tab"), n(Some(F::Tsv), C::None));
        assert_eq!(ok("x.tsv.gz"), n(Some(F::Tsv), C::Gzip));
        assert_eq!(ok(".json"), n(Some(F::Json), C::None));
        // The last format extension wins, as before.
        assert_eq!(ok("x.tsv.json"), n(Some(F::Json), C::None));
        // No format: the caller decides (-O, or a documented default).
        assert_eq!(ok("x.txt"), n(None, C::None));
        assert_eq!(ok("x"), n(None, C::None));
        assert_eq!(ok("x.json."), n(None, C::None));
        assert_eq!(ok("x.gz"), n(None, C::Gzip));
        assert_eq!(ok("x.targz"), n(None, C::None));
        assert_eq!(ok("-"), n(None, C::None));
        assert_eq!(ok("/dev/null"), n(None, C::None));
        assert_eq!(ok("/"), n(None, C::None));
        // Words that only end like an extension are not extensions.
        assert_eq!(ok("x.jsonl"), n(Some(F::Jsonl), C::None));
        assert_eq!(ok("results_json"), n(None, C::None));
    }

    #[test]
    fn unsupported_compression_is_refused() {
        for (name, suffix) in [
            ("regions.tsv.bz2", ".bz2"),
            ("r.json.xz", ".xz"),
            ("x.json.ZIP", ".ZIP"),
            ("x.bz2", ".bz2"),
            ("x.json.Z", ".Z"),
            ("x.json.lz4", ".lz4"),
            ("x.json.bgz", ".bgz"),
            ("x.tgz", ".tgz"),
            ("x.TAR.gz", ".TAR.gz"),
            ("x.json.tar.zst", ".tar.zst"),
            ("x.tar", ".tar"),
        ] {
            match err(name) {
                NameError::UnsupportedCompression { suffix: s, .. } => {
                    assert_eq!(s, suffix, "{name}");
                }
                e => panic!("{name}: {e:?}"),
            }
        }
        let (msg, hint) = err("regions.tsv.bz2").describe(Path::new("regions.tsv.bz2"));
        assert_eq!(
            msg,
            "'regions.tsv.bz2' asks for bzip2 compression, which DiMA does not write"
        );
        assert_eq!(
            hint,
            "DiMA compresses with gzip or Zstandard: name it regions.tsv.gz or regions.tsv.zst"
        );
        let (_, hint) = err("x.json.zstd").describe(Path::new("x.json.zstd"));
        assert_eq!(hint, "DiMA writes this as .zst: name it x.json.zst");
        let (msg, hint) = err("x.tgz").describe(Path::new("x.tgz"));
        assert_eq!(
            msg,
            "'x.tgz' asks for a compressed tar archive, which DiMA does not write"
        );
        assert!(hint.contains("results.json.gz"), "{hint}");
    }

    #[test]
    fn two_compressions_are_refused() {
        assert_eq!(
            err("x.json.gz.zst"),
            NameError::TwoCompressions {
                first: ".gz".into(),
                second: ".zst".into()
            }
        );
        let (msg, hint) = err("x.json.gz.zst").describe(Path::new("x.json.gz.zst"));
        assert_eq!(
            msg,
            "'x.json.gz.zst' asks for two compressions (.gz, then .zst); DiMA applies one"
        );
        assert_eq!(hint, "name it x.json.gz or x.json.zst");
        // An unsupported inner compression under a supported outer one is two compressions too.
        assert!(matches!(
            err("x.json.BZ2.gz"),
            NameError::TwoCompressions { ref first, ref second } if first == ".BZ2" && second == ".gz"
        ));
    }

    #[test]
    fn folders_are_refused() {
        assert_eq!(err("out.json/"), NameError::Folder);
        assert_eq!(err("out/"), NameError::Folder);
        // A last part `.` or `..` names a folder too, though `Path::file_name` skips it.
        for name in ["out.json/.", "out.json/..", ".", "..", "a/b/.."] {
            assert_eq!(err(name), NameError::Folder, "{name}");
        }
        assert_eq!(ok("a.json"), ok("./a.json"));
        assert_eq!(ok("..json").format, Some(F::Json));
        let (msg, _) = err("out.json/").describe(Path::new("out.json/"));
        assert_eq!(msg, "'out.json/' names a folder, not a file");
    }

    /// Names that differ only in letter case or Unicode form fold alike, and are one file
    /// only where the file system says so.
    #[test]
    fn names_that_differ_only_in_case_or_form() {
        use super::{folded, one_file};
        assert_eq!(
            folded(Path::new("Caf\u{e9}.JSON")),
            folded(Path::new("cafe\u{301}.json"))
        );
        let dir = tempfile::tempdir().unwrap();
        let (upper, lower) = (dir.path().join("R.json"), dir.path().join("r.json"));
        assert_eq!(one_file(&upper, &lower), None, "neither exists yet");
        std::fs::write(&upper, "{}").unwrap();
        if std::fs::metadata(&lower).is_ok() {
            // A file system that ignores case (macOS, Windows): one file, where inodes say so.
            assert_ne!(one_file(&upper, &lower), Some(false));
        } else {
            assert_eq!(one_file(&upper, &lower), Some(false), "only one exists");
            std::fs::write(&lower, "{}").unwrap();
            #[cfg(unix)]
            assert_eq!(one_file(&upper, &lower), Some(false), "two inodes");
        }
    }

    #[test]
    fn extensions_read_back() {
        for f in [F::Json, F::Jsonl, F::Tsv] {
            for ext in f.extensions() {
                for c in [C::None, C::Gzip, C::Zstd] {
                    let name = format!("x{ext}{}", c.extension());
                    assert_eq!(
                        ok(&name),
                        Named {
                            format: Some(f),
                            compression: c
                        },
                        "{name}"
                    );
                }
            }
        }
    }
}
