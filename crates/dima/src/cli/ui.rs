//! Messages on standard error (D98): `error:`, `warning[code]:`, indented `caused by:`,
//! `e.g.` and `hint:` lines in clap's style, colour only as reinforcement (WCAG 1.4.1), and the
//! end-of-run summary.

use std::borrow::Cow;
use std::io::Write;

use anstyle::{AnsiColor, Style};
use dima_core::DimaError;
use dima_core::diagnostics::{Diagnostic, Severity};

const ERROR: Style = AnsiColor::Red.on_default().bold();
const WARNING: Style = AnsiColor::Yellow.on_default().bold();
const NOTE: Style = AnsiColor::Cyan.on_default().bold();
const HINT: Style = AnsiColor::Green.on_default().bold();
const DIM: Style = Style::new().dimmed();

/// What to print.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verbosity {
    /// `-q` given (errors only).
    pub quiet: bool,
    /// Number of `-v`.
    pub verbose: u8,
}

impl Verbosity {
    /// Warnings and the summary line are printed.
    #[must_use]
    pub const fn normal(self) -> bool {
        !self.quiet
    }

    /// Milestones and the performance report are printed.
    #[must_use]
    pub const fn verbose(self) -> bool {
        !self.quiet && self.verbose >= 1
    }
}

/// Standard error, with the progress line (if any) erased first so messages start on a clean
/// line; the display redraws it afterwards.
fn stderr() -> anstream::AutoStream<std::io::Stderr> {
    super::progress::clear();
    anstream::AutoStream::auto(std::io::stderr())
}

/// Text from the input or the file system (headers, characters, file names) made safe for a
/// terminal: control characters (which could move the cursor, change colours or retitle the
/// window) and bidirectional overrides (which reorder what is displayed) are shown as escapes
/// such as `\u{1b}`. Line breaks and tabs are kept.
fn safe(text: &str) -> Cow<'_, str> {
    let unsafe_char = |c: char| {
        (c.is_control() && c != '\n' && c != '\t')
            || matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
    };
    if !text.chars().any(unsafe_char) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        if unsafe_char(c) {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

/// Prints an error with its causes and hint (always, even with `-q`).
pub fn error(e: &DimaError) {
    let mut w = stderr();
    let _ = writeln!(w, "{ERROR}error:{ERROR:#} {}", safe(&e.to_string()));
    let mut cause = std::error::Error::source(e);
    while let Some(c) = cause {
        let _ = writeln!(w, "  {DIM}caused by:{DIM:#} {}", safe(&c.to_string()));
        cause = c.source();
    }
    if let Some(h) = e.hint() {
        let _ = writeln!(w, "  {HINT}hint:{HINT:#} {}", safe(h));
    }
}

/// Prints a plain error message (usage problems found by the CLI itself).
pub fn error_message(message: &str, hint: Option<&str>) {
    let mut w = stderr();
    let _ = writeln!(w, "{ERROR}error:{ERROR:#} {}", safe(message));
    if let Some(h) = hint {
        let _ = writeln!(w, "  {HINT}hint:{HINT:#} {}", safe(h));
    }
}

/// Prints a diagnostic: warnings at normal verbosity, informational ones at `-v`.
pub fn diagnostic(d: &Diagnostic, v: Verbosity) {
    let show = match d.severity {
        Severity::Warning => v.normal(),
        Severity::Info => v.verbose(),
    };
    if !show {
        return;
    }
    let mut w = stderr();
    let (style, label) = match d.severity {
        Severity::Warning => (WARNING, "warning"),
        Severity::Info => (NOTE, "note"),
    };
    let _ = writeln!(
        w,
        "{style}{label}[{}]:{style:#} {}",
        d.code,
        safe(&d.message)
    );
    if !d.examples.is_empty() {
        let shown: Vec<&str> = d.examples.iter().take(3).map(String::as_str).collect();
        // One example is kept per distinct item (up to five), so more examples than shown
        // means more items; the count can be larger without them (one value in two records).
        let more = if d.examples.len() > shown.len() {
            "; and more"
        } else {
            ""
        };
        let _ = writeln!(w, "  {DIM}e.g.{DIM:#} {}{more}", safe(&shown.join("; ")));
    }
    if let Some(h) = &d.hint {
        let _ = writeln!(w, "  {HINT}hint:{HINT:#} {}", safe(h));
    }
}

/// Prints a warning raised by the CLI itself.
pub fn warning(code: &str, message: &str, hint: Option<&str>, v: Verbosity) {
    if !v.normal() {
        return;
    }
    let mut w = stderr();
    let _ = writeln!(w, "{WARNING}warning[{code}]:{WARNING:#} {}", safe(message));
    if let Some(h) = hint {
        let _ = writeln!(w, "  {HINT}hint:{HINT:#} {}", safe(h));
    }
}

/// Prints an informational line at normal verbosity.
pub fn note(message: &str, v: Verbosity) {
    if v.normal() {
        let _ = writeln!(stderr(), "{NOTE}note:{NOTE:#} {}", safe(message));
    }
}

/// Prints a line at `-v`.
pub fn verbose(message: &str, v: Verbosity) {
    if v.verbose() {
        let _ = writeln!(stderr(), "{DIM}{}{DIM:#}", safe(message));
    }
}

/// Prints a line at normal verbosity, unstyled (the summary line).
pub fn line(message: &str, v: Verbosity) {
    if v.normal() {
        let _ = writeln!(stderr(), "{}", safe(message));
    }
}

#[cfg(test)]
mod tests {
    use super::safe;

    #[test]
    fn terminal_controls_are_escaped() {
        assert!(matches!(
            safe("plain text\twith tab\n"),
            std::borrow::Cow::Borrowed(_)
        ));
        assert_eq!(safe("a\u{1b}]0;title\u{7}b"), "a\\u{1b}]0;title\\u{7}b");
        assert_eq!(safe("x\u{9b}31m"), "x\\u{9b}31m");
        assert_eq!(safe("abc\u{202e}fed"), "abc\\u{202e}fed");
        assert_eq!(safe("Hôte é"), "Hôte é");
    }
}
