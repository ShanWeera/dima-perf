//! `dima`: the DiMA command-line interface (Tharanga et al. 2025, doi:10.1093/bib/bbae607).

use std::io::Write;
use std::process::ExitCode;

/// musl's allocator is slow under multi-threaded allocation (D92).
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> ExitCode {
    // A panic is a bug: report it plainly (D62). Unwinding drops the output sinks, which removes
    // their temporary files; the process then exits with status 1.
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map_or_else(String::new, |l| format!(" at {}:{}", l.file(), l.line()));
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_owned());
        // Never `eprintln!` here: a panic while panicking (e.g. stderr closed) aborts.
        let _ = writeln!(
            std::io::stderr(),
            "error: internal error: this is a bug in DiMA; please report it at {}/issues\n  \
             details: {message}{location}\n  version: dima {} ({})\n  hint: set RUST_BACKTRACE=1 \
             to include a backtrace in the report",
            env!("CARGO_PKG_REPOSITORY"),
            env!("CARGO_PKG_VERSION"),
            env!("DIMA_TARGET"),
        );
        if std::env::var_os("RUST_BACKTRACE").is_some() {
            let _ = writeln!(
                std::io::stderr(),
                "{}",
                std::backtrace::Backtrace::force_capture()
            );
        }
    }));
    std::panic::catch_unwind(dima::cli::main).unwrap_or_else(|_| ExitCode::from(1))
}
