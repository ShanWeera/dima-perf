//! The `dima` command-line interface.

pub mod analyze;
pub mod args;
pub mod cite;
pub mod hcs;
pub mod help;
pub mod outputs;
pub mod progress;
pub mod resources;
pub mod signals;
pub mod threads;
pub mod ui;
pub mod view;

use std::io::{IsTerminal, Write};
use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use dima_core::{DimaError, Result};

use args::{Cli, Command};
use ui::Verbosity;

/// Build target of this binary (set by build.rs), recorded in run reports.
pub const TARGET: &str = env!("DIMA_TARGET");

/// DiMA's own licence.
const LICENSE: &str = include_str!("../../../../LICENSE");
/// Licences of everything compiled into it.
const NOTICES: &str = include_str!("../../../../THIRD-PARTY-NOTICES.md");

/// State shared by the subcommands.
pub struct Context {
    /// What to print.
    pub verbosity: Verbosity,
    /// Interrupt handling.
    pub signals: signals::Signals,
    /// Standard error is a terminal.
    pub stderr_is_terminal: bool,
    /// Standard error is a terminal that can redraw a line (for the progress line).
    pub redraw_terminal: bool,
    /// Errors that followed the command's own error (writing the run report of a failed run),
    /// printed after it.
    pub later_errors: std::cell::RefCell<Vec<DimaError>>,
}

impl Context {
    /// `Err(Cancelled)` after an interrupt.
    ///
    /// # Errors
    /// [`DimaError::Cancelled`].
    pub fn signals_check(&self) -> Result<()> {
        if self
            .signals
            .cancel
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            Err(DimaError::Cancelled)
        } else {
            Ok(())
        }
    }
}

fn init_logging(v: Verbosity) {
    let filter = std::env::var("DIMA_LOG").ok().map_or_else(
        || {
            let level = match (v.quiet, v.verbose) {
                (true, _) => "error",
                (false, 0 | 1) => "warn",
                (false, _) => "dima=debug,dima_core=debug",
            };
            tracing_subscriber::EnvFilter::new(level)
        },
        tracing_subscriber::EnvFilter::new,
    );
    let ansi = anstream::AutoStream::choice(&std::io::stderr()) != anstream::ColorChoice::Never;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(|| {
            progress::clear();
            std::io::stderr()
        })
        .with_ansi(ansi)
        .with_target(false)
        .without_time()
        .try_init();
}

/// `FORCE_COLOR` set to any non-empty value (force-color.org), unless `NO_COLOR` is: the
/// colour order of D98 is `--color`, `NO_COLOR`, `FORCE_COLOR`/`CLICOLOR_FORCE`, `CLICOLOR`,
/// then the terminal. anstream reads every variable but `FORCE_COLOR`.
fn colour_forced() -> bool {
    let set = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    set("FORCE_COLOR") && !set("NO_COLOR")
}

/// Makes `choice` the colour decision of every writer: `auto` leaves it to the environment
/// (anstream), where `FORCE_COLOR` also forces colour.
fn write_colour(choice: clap::ColorChoice) {
    let choice = match choice {
        clap::ColorChoice::Auto if colour_forced() => clap::ColorChoice::Always,
        c => c,
    };
    colorchoice_clap::Color { color: choice }.write_global();
}

/// The `--color` value on the command line (the last one), read before parsing so that usage
/// errors, which clap prints while parsing, honour it too.
fn colour_arg(args: impl Iterator<Item = std::ffi::OsString>) -> Option<clap::ColorChoice> {
    use clap::ValueEnum;
    let mut args = args.skip(1);
    let mut found = None;
    while let Some(arg) = args.next() {
        let value = match arg.to_str() {
            Some("--") => break,
            Some("--color") => args.next().and_then(|v| v.into_string().ok()),
            Some(a) => match a.strip_prefix("--color=") {
                Some(v) => Some(v.to_owned()),
                None => continue,
            },
            None => continue,
        };
        if let Some(c) = value.and_then(|v| clap::ColorChoice::from_str(&v, true).ok()) {
            found = Some(c);
        }
    }
    found
}

/// A terminal that cannot redraw a line: `TERM=dumb` (Emacs shells, some CI consoles), or no
/// `TERM` at all on Unix (Windows consoles never set it).
fn dumb_terminal() -> bool {
    match std::env::var_os("TERM") {
        Some(t) => t == "dumb",
        None => cfg!(unix),
    }
}

/// Parses the command line and runs the subcommand; returns the process exit code.
pub fn main() -> ExitCode {
    // Before parsing too, so that `--help` and usage errors follow the same choice.
    write_colour(colour_arg(std::env::args_os()).unwrap_or(clap::ColorChoice::Auto));
    let cli = Cli::parse();
    write_colour(cli.global.color.color);
    let verbosity = Verbosity {
        quiet: cli.global.quiet > 0,
        verbose: cli.global.verbose,
    };
    init_logging(verbosity);
    if cli.licenses {
        let _ = write!(std::io::stdout(), "{LICENSE}\n{NOTICES}");
        return ExitCode::SUCCESS;
    }
    let Some(command) = cli.command else {
        // A usage error: the help goes to standard error, like clap's own usage errors.
        let _ = write!(
            anstream::stderr(),
            "{}",
            Cli::command().render_help().ansi()
        );
        return ExitCode::from(2);
    };
    let stderr_is_terminal = std::io::stderr().is_terminal();
    let signals = match signals::install(verbosity.normal()) {
        Ok(s) => s,
        Err(e) => {
            let e = DimaError::system("the interrupt handler", e);
            ui::error(&e);
            return ExitCode::from(e.class().code());
        }
    };
    let redraw_terminal = stderr_is_terminal && !dumb_terminal();
    let ctx = Context {
        verbosity,
        signals,
        stderr_is_terminal,
        redraw_terminal,
        later_errors: std::cell::RefCell::default(),
    };
    let result = match &command {
        Command::Analyze(a) => analyze::run(a, &ctx),
        Command::View(a) => view::run(a, &ctx),
        Command::Hcs(a) => hcs::run(a, &ctx),
        Command::Cite(a) => cite::run(a),
        Command::Completions { shell } => completions(*shell),
    };
    finish(result, &ctx)
}

/// Writes a completion script. It is rendered into memory first: clap_complete panics on write
/// errors, and `dima completions bash | head` must end quietly like every other command.
fn completions(shell: clap_complete::Shell) -> Result<()> {
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut Cli::command(), "dima", &mut script);
    let mut out = std::io::stdout().lock();
    out.write_all(&script)
        .and_then(|()| out.flush())
        .map_err(|e| {
            DimaError::io(
                dima_core::error::IoAction::Write,
                dima_core::error::IoTarget::Stdout,
                e,
            )
        })
}

fn finish(result: Result<()>, ctx: &Context) -> ExitCode {
    match result {
        Ok(()) => {
            signals::cleanup_done();
            ExitCode::SUCCESS
        }
        Err(e) if e.is_broken_pipe() => {
            // `dima … | head`: the reader has what it wanted (ripgrep's convention).
            signals::cleanup_done();
            ExitCode::SUCCESS
        }
        Err(DimaError::Cancelled) => {
            signals::cleanup_done();
            if ctx.verbosity.normal() {
                ui::error_message("cancelled; no results were written", None);
            }
            for later in ctx.later_errors.take() {
                ui::error(&later);
            }
            ctx.signals.terminate()
        }
        Err(e) => {
            ui::error(&e);
            for later in ctx.later_errors.take() {
                ui::error(&later);
            }
            signals::cleanup_done();
            ExitCode::from(e.class().code())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(args: &[&str]) -> Option<clap::ColorChoice> {
        colour_arg(args.iter().map(std::ffi::OsString::from))
    }

    #[test]
    fn colour_is_read_before_parsing() {
        assert_eq!(arg(&["dima", "analyze", "--bogus"]), None);
        assert_eq!(
            arg(&["dima", "--color", "never", "analyze", "--bogus"]),
            Some(clap::ColorChoice::Never)
        );
        assert_eq!(
            arg(&["dima", "analyze", "--color=ALWAYS", "-k"]),
            Some(clap::ColorChoice::Always)
        );
        // The last one counts; invalid values and operands after `--` are not options.
        assert_eq!(
            arg(&[
                "dima",
                "--color=never",
                "--color",
                "auto",
                "--color",
                "pink"
            ]),
            Some(clap::ColorChoice::Auto)
        );
        assert_eq!(arg(&["dima", "view", "--", "--color=never"]), None);
        assert_eq!(arg(&["dima", "--color"]), None);
    }
}
