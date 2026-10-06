//! The `dima` command-line interface as a library, so that `xtask` can generate the CLI
//! reference and man pages from the same definitions the binary parses (D94). The binary
//! (`src/main.rs`) only installs the panic report and calls [`cli::main`].

pub mod cli;
