//! The command-line reference page, rendered from the same clap definitions the binary parses,
//! so the documentation can never disagree with `dima --help`.

use clap::{Arg, ArgAction, Command, CommandFactory};

use crate::markdown::{escape, linkify};

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
    if matches!(arg.get_action(), ArgAction::Count) {
        sig.push_str(" (repeatable)");
    }
    sig
}

/// Help text as Markdown paragraphs; the trailing `Ref:` paragraph becomes a reference line.
fn help_text(arg: &Arg) -> String {
    let text = arg
        .get_long_help()
        .or_else(|| arg.get_help())
        .map(ToString::to_string)
        .unwrap_or_default();
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

fn details(arg: &Arg) -> Vec<String> {
    let mut d = Vec::new();
    let values: Vec<String> = arg
        .get_possible_values()
        .iter()
        .filter(|v| !v.is_hide_set())
        .map(|v| format!("`{}`", v.get_name()))
        .collect();
    if !values.is_empty() && !matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::Count) {
        d.push(format!("Values: {}", values.join(", ")));
    }
    let defaults: Vec<String> = arg
        .get_default_values()
        .iter()
        .map(|v| format!("`{}`", v.to_string_lossy()))
        .collect();
    if !defaults.is_empty() && !matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::Count) {
        d.push(format!("Default: {}", defaults.join(", ")));
    }
    if let Some(env) = arg.get_env() {
        d.push(format!("Environment: `{}`", env.to_string_lossy()));
    }
    let aliases: Vec<String> = arg
        .get_visible_aliases()
        .unwrap_or_default()
        .iter()
        .map(|a| format!("`--{a}`"))
        .collect();
    if !aliases.is_empty() {
        d.push(format!("Aliases: {}", aliases.join(", ")));
    }
    d
}

fn command_section(out: &mut String, cmd: &Command, title: &str) {
    out.push_str(&format!("## `{title}`\n\n"));
    if let Some(about) = cmd.get_long_about().or_else(|| cmd.get_about()) {
        let about = about.to_string();
        for p in about.split("\n\n") {
            let p = p.split_whitespace().collect::<Vec<_>>().join(" ");
            match p.strip_prefix("Ref: ") {
                Some(refs) => out.push_str(&format!("**References:** {}\n\n", linkify(refs))),
                None => out.push_str(&format!("{}\n\n", linkify(&p))),
            }
        }
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
    // Global options are documented once, under `dima`.
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
    if !is_root {
        out.push_str("Global options (`--color`, `-q`, `-v`) apply to every subcommand; see [`dima`](#dima).\n\n");
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
        "This page is generated from the definitions `dima` itself parses, so it always matches \
         `dima --help` for this version. Run `dima <command> --help` for the same text in a \
         terminal; `-h` shows one-line summaries.\n\n",
    );
    out.push_str(
        "| Exit status | Meaning |\n|---|---|\n| `0` | Success (also when a reader of standard output, such as `head`, stops early) |\n| `1` | Internal error (a bug; please report it) |\n| `2` | Usage error (invalid options) |\n| `3` | Input/output or system error (missing file, no permission, disk full, a thread the system refused) |\n| `4` | Invalid input data (not an aligned FASTA, unequal lengths, invalid characters in strict mode) |\n| `130`, `143`, `129` | Interrupted by SIGINT (Ctrl-C), SIGTERM or SIGHUP; unfinished outputs are removed, never left partial |\n\n",
    );
    command_section(&mut out, &cmd, "dima");
    for sub in cmd.get_subcommands().filter(|s| s.get_name() != "help") {
        command_section(&mut out, sub, &format!("dima {}", sub.get_name()));
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}
