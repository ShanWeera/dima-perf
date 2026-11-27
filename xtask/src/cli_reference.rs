//! The command-line reference page, rendered from the same clap definitions the binary parses,
//! so every option, value and default on it is the one `dima --help` shows. The page adds what
//! the help cannot show: the meaning of each value, the spellings `dima` accepts without showing
//! them, the environment variables behind some options, and the exit statuses.

use clap::{Arg, ArgAction, Command, CommandFactory};

use crate::markdown::{escape, linkify};

/// The environment variables that change what an option does when it is not given, with the
/// page that explains their order.
const ENVIRONMENT: &[(&str, &[&str])] = &[
    (
        "color",
        &["NO_COLOR", "FORCE_COLOR", "CLICOLOR_FORCE", "CLICOLOR"],
    ),
    ("verbose", &["DIMA_LOG"]),
    ("progress", &["DIMA_PROGRESS", "DIMA_NO_PROGRESS"]),
    (
        "threads",
        &[
            "RAYON_NUM_THREADS",
            "SLURM_CPUS_ON_NODE",
            "PBS_NUM_PPN",
            "PE_HOSTFILE",
            "LSB_MCPU_HOSTS",
        ],
    ),
];

/// `-k, --kmer-length <K>`.
fn signature(arg: &Arg) -> String {
    let mut parts = Vec::new();
    if let Some(s) = arg.get_short() {
        parts.push(format!("-{s}"));
    }
    if let Some(l) = arg.get_long() {
        parts.push(format!("--{l}"));
    }
    let takes_value = matches!(arg.get_action(), ArgAction::Set | ArgAction::Append);
    let value = arg
        .get_value_names()
        .map(|v| {
            v.iter()
                .map(|n| format!("<{n}>"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|| format!("<{}>", arg.get_id().as_str().to_uppercase()));
    let mut sig = parts.join(", ");
    if arg.is_positional() {
        sig = value;
    } else if takes_value {
        sig.push(' ');
        sig.push_str(&value);
    }
    sig
}

/// A help text as Markdown paragraphs; the trailing `Ref:` paragraph becomes a reference line.
fn paragraphs(text: &str) -> String {
    text.split("\n\n")
        .map(|p| {
            let p = p.split_whitespace().collect::<Vec<_>>().join(" ");
            match p.strip_prefix("Ref: ") {
                Some(refs) => format!("**References:** {}", linkify(refs)),
                None => linkify(&p),
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn help_text(arg: &Arg) -> String {
    let text = arg
        .get_long_help()
        .or_else(|| arg.get_help())
        .map(ToString::to_string)
        .unwrap_or_default();
    paragraphs(&text)
}

fn details(arg: &Arg) -> Vec<String> {
    let mut d = Vec::new();
    let takes_value = !matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::Count);
    let values: Vec<String> = arg
        .get_possible_values()
        .iter()
        .filter(|v| !v.is_hide_set())
        .map(|v| {
            let mut line = format!("`{}`", v.get_name());
            let aliases: Vec<String> = v
                .get_name_and_aliases()
                .skip(1)
                .map(|a| format!("`{a}`"))
                .collect();
            if !aliases.is_empty() {
                line.push_str(&format!(" (also accepted: {})", aliases.join(", ")));
            }
            if let Some(help) = v.get_help() {
                line.push_str(&format!(
                    ": {}",
                    escape(help.to_string().trim_end_matches('.'))
                ));
            }
            line
        })
        .collect();
    if !values.is_empty() && takes_value {
        d.push(format!(
            "Values:\n{}",
            values
                .iter()
                .map(|v| format!("  - {v}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    let defaults: Vec<String> = arg
        .get_default_values()
        .iter()
        .map(|v| format!("`{}`", v.to_string_lossy()))
        .collect();
    if !defaults.is_empty() && takes_value {
        d.push(format!("Default: {}", defaults.join(", ")));
    }
    if let Some((_, vars)) = ENVIRONMENT
        .iter()
        .find(|(id, _)| *id == arg.get_id().as_str())
    {
        let vars: Vec<String> = vars.iter().map(|v| format!("`{v}`")).collect();
        d.push(format!(
            "Environment: {} (see [Environment variables](/reference/environment/))",
            vars.join(", ")
        ));
    }
    let aliases: Vec<String> = arg
        .get_all_aliases()
        .unwrap_or_default()
        .iter()
        .map(|a| format!("`--{a}`"))
        .collect();
    if !aliases.is_empty() {
        d.push(format!("Also accepted: {}", aliases.join(", ")));
    }
    d
}

fn command_section(out: &mut String, cmd: &Command, title: &str) {
    out.push_str(&format!("## `{title}`\n\n"));
    if let Some(about) = cmd.get_long_about().or_else(|| cmd.get_about()) {
        out.push_str(&paragraphs(&about.to_string()));
        out.push_str("\n\n");
    }
    let aliases: Vec<String> = cmd
        .get_all_aliases()
        .map(|a| format!("`{}`", title.replacen(cmd.get_name(), a, 1)))
        .collect();
    if !aliases.is_empty() {
        out.push_str(&format!("Also accepted as {}.\n\n", aliases.join(", ")));
    }
    let usage = cmd.clone().render_usage().to_string();
    let usage = usage.trim_start_matches("Usage: ").trim();
    out.push_str(&format!("```text\n{usage}\n```\n\n"));
    // Options grouped by their help heading, in order of first appearance.
    let mut groups: Vec<(String, Vec<&Arg>)> = Vec::new();
    for arg in cmd.get_arguments() {
        let id = arg.get_id().as_str();
        if arg.is_hide_set() || id == "help" || id == "version" {
            continue;
        }
        let heading = arg.get_help_heading().unwrap_or("Options").to_owned();
        match groups.iter_mut().find(|(h, _)| *h == heading) {
            Some((_, args)) => args.push(arg),
            None => groups.push((heading, vec![arg])),
        }
    }
    // Global options, and help and version, are documented once, under `dima`.
    let is_root = title == "dima";
    for (heading, args) in groups {
        if heading == "Global options" && !is_root {
            continue;
        }
        out.push_str(&format!("### {}\n\n", escape(&heading)));
        for arg in args {
            out.push_str(&format!("#### `{}`\n\n", signature(arg)));
            let help = help_text(arg);
            if !help.is_empty() {
                out.push_str(&help);
                out.push_str("\n\n");
            }
            let d = details(arg);
            if !d.is_empty() {
                for line in d {
                    out.push_str(&format!("- {line}\n"));
                }
                out.push('\n');
            }
        }
    }
    // Options kept for older scripts: accepted, not shown by --help.
    let hidden: Vec<&Arg> = cmd.get_arguments().filter(|a| a.is_hide_set()).collect();
    if !hidden.is_empty() {
        out.push_str("### Also accepted\n\nNot shown by `--help`; kept for older scripts.\n\n");
        for arg in hidden {
            out.push_str(&format!("- `{}`: {}\n", signature(arg), help_text(arg)));
        }
        out.push('\n');
    }
    if is_root {
        let version = dima_core::citations::SOFTWARE_VERSION;
        out.push_str(&format!(
            "### Help and version\n\n\
             Every command also takes these options.\n\n\
             #### `-h, --help`\n\n\
             `-h` prints a one-line summary of each option, `--help` the full texts shown on this \
             page. `dima help` and `dima help COMMAND` print the same as `dima --help` and \
             `dima COMMAND --help`. Help is printed on standard output, with status 0.\n\n\
             #### `-V, --version`\n\n\
             Prints the version (`dima {version}`) and exits with status 0. After a subcommand \
             it names that subcommand: `dima analyze --version` prints \
             `dima-analyze {version}`.\n\n",
        ));
    } else {
        out.push_str(
            "Global options (`--color`, `-q`, `-v`) and `-h`/`--help`, `-V`/`--version` apply to \
             every subcommand; see [`dima`](#dima).\n\n",
        );
    }
}

/// The whole page.
pub fn render() -> String {
    let mut cmd = dima::cli::args::Cli::command();
    cmd.build();
    let mut out = String::from(
        "---\ntitle: Command-line reference\ndescription: Every dima subcommand and option, generated from the program's own definitions.\n---\n\n<!-- Generated by `cargo xtask gen` from crates/dima/src/cli; do not edit. -->\n\n",
    );
    out.push_str(
        "This page is generated from the definitions `dima` itself parses, so every option, \
         value and default on it is the one `dima --help` shows for this version. It also lists \
         what the help does not show: the meaning of each value, the spellings `dima` accepts \
         without showing them, and the environment variables behind some options. Run \
         `dima <command> --help` for the same texts in a terminal; `-h` shows one-line \
         summaries. Option values are lower case and case-sensitive.\n\n",
    );
    out.push_str(
        "| Exit status | Meaning |\n|---|---|\n\
         | `0` | Success (also when a reader of standard output, such as `head`, stops early) |\n\
         | `1` | Internal error (a bug; please report it) |\n\
         | `2` | Usage error: invalid options, a name DiMA cannot write, two outputs on one file, a folder given as a file; also `dima` without arguments |\n\
         | `3` | Input/output or system error (missing file, no permission, disk full, a thread the system refused) |\n\
         | `4` | Invalid input data (not an aligned FASTA, unequal lengths, invalid characters in strict mode; a results file of another major schema version, or edited or damaged) |\n\
         | 128 + N | Interrupted by signal N: `130` SIGINT (Ctrl-C), `143` SIGTERM, `129` SIGHUP; on Linux also `138` SIGUSR1, `140` SIGUSR2, `142` SIGALRM, `152` SIGXCPU (on macOS SIGUSR1 and SIGUSR2 give `158` and `159`). Unfinished files are removed, never left partial; standard output may hold a partial document |\n\n\
         See [Exit codes](/reference/exit-codes/) for examples and scripts.\n\n",
    );
    command_section(&mut out, &cmd, "dima");
    for sub in cmd.get_subcommands() {
        command_section(&mut out, sub, &format!("dima {}", sub.get_name()));
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    /// Every variable the page names for an option is explained on the page it links to.
    #[test]
    fn environment_variables_are_documented() {
        let page = std::fs::read_to_string(
            crate::root().join("docs/src/content/docs/reference/environment.md"),
        )
        .unwrap();
        for (option, vars) in super::ENVIRONMENT {
            for v in *vars {
                assert!(page.contains(&format!("`{v}`")), "{option}: {v}");
            }
        }
    }

    #[test]
    fn hidden_spellings_and_value_meanings_are_listed() {
        let page = super::render();
        for expected in [
            "Also accepted as `dima analyse`.",
            "Also accepted: `--name`",
            "`protein` (also accepted: `aa`, `amino-acid`): Amino acids",
            "`dna` (also accepted: `nucleotide`)",
            "`raw` (also accepted: `plug-in`)",
            "`report` (also accepted: `report-only`)",
            "`metadata`: One row per variant and header value",
            "- `--no-progress`: The same as \\`--progress off\\`",
            "#### `-h, --help`",
            "#### `-V, --version`",
            "## `dima help`",
            "Environment: `DIMA_PROGRESS`, `DIMA_NO_PROGRESS`",
            "| 128 + N |",
        ] {
            assert!(page.contains(expected), "{expected:?}");
        }
        assert!(!page.contains("(repeatable)"));
    }
}
