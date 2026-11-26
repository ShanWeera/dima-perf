//! Consistency checks that are not generated files.

use std::path::Path;

use dima_core::citations;
use serde_json::Value;

/// Every check; returns the problems found.
pub fn all(root: &Path) -> Vec<String> {
    let mut problems = crossref_snapshot(root);
    problems.extend(uncited_references(root));
    problems.extend(tier_tags(root));
    problems.extend(schema_versions(root));
    problems.extend(changelog(root));
    problems.extend(release_versions(root));
    problems
}

/// Every registry entry is cited somewhere: by key in the code (`Cite::new("key")`,
/// `Reference::of("key")`, key lists) or as `[@key` in the docs. The bibliography lists "every
/// reference cited by DiMA's help, desktop app and documentation", so an entry nothing cites
/// would be listed falsely.
fn uncited_references(root: &Path) -> Vec<String> {
    let mut code = String::new();
    let mut docs = String::new();
    collect(&root.join("crates"), &["rs"], &mut code);
    collect(
        &root.join("docs/src"),
        &["md", "mdx", "astro", "ts", "mjs"],
        &mut docs,
    );
    citations::all()
        .iter()
        .filter(|c| {
            !(code.contains(&format!("\"{}\"", c.key)) || docs.contains(&format!("@{}", c.key)))
        })
        .map(|c| {
            format!(
                "citation `{}` is cited nowhere (code or docs): cite it or remove it from the \
                 registry",
                c.key
            )
        })
        .collect()
}

/// The *Design rationale* page labels every citation by its tier, by hand: each citation group
/// `[@key, locator; @key2]` of a [`citations::Tier::General`] or [`citations::Tier::Standard`]
/// reference is followed by its tag (` [general]`, ` [standard]`), a group of domain references
/// by none, a group never mixes tiers, and no tag stands anywhere else.
fn tier_tags(root: &Path) -> Vec<String> {
    let page = "docs/src/content/docs/explanation/design-rationale.md";
    let Ok(text) = std::fs::read_to_string(root.join(page)) else {
        return vec![format!("{page}: cannot be read")];
    };
    tag_problems(&text)
        .into_iter()
        .map(|p| format!("{page}: {p}"))
        .collect()
}

fn tag_problems(text: &str) -> Vec<String> {
    const TAGS: [&str; 2] = [" [general]", " [standard]"];
    let mut problems = Vec::new();
    let mut tagged = 0;
    let mut rest = text;
    while let Some(open) = rest.find("[@") {
        let Some(len) = rest[open..].find(']') else {
            problems.push("an unclosed citation group".to_owned());
            break;
        };
        let group = &rest[open + 1..open + len];
        let after = &rest[open + len + 1..];
        let tag = TAGS
            .iter()
            .find(|t| after.starts_with(*t))
            .map(|t| t.trim());
        tagged += usize::from(tag.is_some());
        let tiers: std::collections::BTreeSet<&str> = group
            .split(';')
            .filter_map(|part| {
                let key = part.trim().strip_prefix('@')?;
                let key = key.split([',', ' ']).next().unwrap_or(key);
                match citations::by_key(key) {
                    Some(c) => Some(c.tier.label()),
                    None => {
                        problems.push(format!("[{group}]: `{key}` is not in the registry"));
                        None
                    }
                }
            })
            .collect();
        match tiers.len() {
            0 => {}
            1 => {
                let want = tiers.first().copied().unwrap_or_default();
                if tag.unwrap_or("") != want {
                    let want = if want.is_empty() { "no tag" } else { want };
                    problems.push(format!(
                        "[{group}] needs {want}, has {}",
                        tag.unwrap_or("no tag")
                    ));
                }
            }
            _ => problems.push(format!("[{group}] mixes tiers; split it")),
        }
        rest = after;
    }
    // Tags outside citation groups (the bold ones in the introduction explain them).
    let all = TAGS
        .iter()
        .map(|t| {
            text.matches(t.trim()).count() - text.matches(&format!("**{}**", t.trim())).count()
        })
        .sum::<usize>();
    if all != tagged {
        problems.push(format!(
            "{} tier tag(s) stand apart from a citation group",
            all.saturating_sub(tagged)
        ));
    }
    problems
}

/// Appends the text of every file under `dir` with one of `extensions`, except the registry
/// data itself.
fn collect(dir: &Path, extensions: &[&str], out: &mut String) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, extensions, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.contains(&e))
            && path.file_name().is_some_and(|n| n != "citations_data.rs")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            out.push_str(&text);
            out.push('\n');
        }
    }
}

/// Version numbers written by hand (install commands, archive names, the citing example, the
/// Bioconda recipe) must be the workspace version, so a release never documents the previous
/// one. Only numbers after these prefixes are checked (others, such as DiMA v5.0.9, are not
/// this software's version).
fn release_versions(root: &Path) -> Vec<String> {
    const FILES: &[&str] = &[
        "docs/src/content/docs/getting-started/install.mdx",
        "docs/src/content/docs/reference/citing.mdx",
        "README.md",
        "packaging/bioconda/meta.yaml",
    ];
    const PREFIXES: &[&str] = &[
        "VERSION=",
        "$Version = \"",
        "--tag v",
        "dima-",
        "DiMA-",
        "dima-gui_",
        "dima-gui-",
        "using dima ",
        "use version ",
        "set version = \"",
    ];
    let want = citations::SOFTWARE_VERSION;
    let mut problems = Vec::new();
    for file in FILES {
        let Ok(text) = std::fs::read_to_string(root.join(file)) else {
            problems.push(format!("{file}: cannot be read"));
            continue;
        };
        let text = text.replace('\n', " ");
        for prefix in PREFIXES {
            for (at, _) in text.match_indices(prefix) {
                let rest = &text[at + prefix.len()..];
                let end = rest
                    .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                    .unwrap_or(rest.len());
                let found = rest[..end].trim_end_matches('.');
                let parts: Vec<&str> = found.split('.').collect();
                let semver = parts.len() == 3 && parts.iter().all(|p| !p.is_empty());
                if semver && found != want {
                    problems.push(format!(
                        "{file}: version {found} after `{prefix}` is not the workspace version {want}"
                    ));
                }
            }
        }
    }
    problems
}

/// The DOI-bearing references must equal the verified Crossref snapshot field for field
/// (`scripts/citations/crossref.py` reports where Crossref now differs from the snapshot).
fn crossref_snapshot(root: &Path) -> Vec<String> {
    let path = root.join("xtask/data/crossref_snapshot.json");
    let snapshot: Value = match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::from_str(&s).map_err(|e| e.to_string()))
    {
        Ok(v) => v,
        Err(e) => return vec![format!("{}: {e}", path.display())],
    };
    let Some(snapshot) = snapshot.as_object() else {
        return vec![format!("{}: not a JSON object", path.display())];
    };
    let mut problems = Vec::new();
    let mut seen = 0usize;
    for c in citations::all() {
        let Some(doi) = c.doi else { continue };
        seen += 1;
        let Some(record) = snapshot.get(doi) else {
            problems.push(format!(
                "{} (doi:{doi}) is missing from the Crossref snapshot",
                c.key
            ));
            continue;
        };
        let authors: Vec<Value> = c
            .authors
            .iter()
            .map(|(f, g)| Value::from(vec![*f, *g]))
            .collect();
        let opt = |v: Option<&str>| v.map_or(Value::Null, Value::from);
        let expected = [
            ("key", Value::from(c.key)),
            ("authors", Value::from(authors)),
            ("year", Value::from(c.year)),
            ("title", Value::from(c.title)),
            ("venue", Value::from(c.venue)),
            ("volume", opt(c.volume)),
            ("issue", opt(c.issue)),
            ("pages", opt(c.pages)),
        ];
        for (field, want) in expected {
            let got = record.get(field).cloned().unwrap_or(Value::Null);
            if got != want {
                problems.push(format!(
                    "{} (doi:{doi}): {field} is {want} in citations_data.rs but {got} in the Crossref snapshot"
                , c.key));
            }
        }
    }
    if seen != snapshot.len() {
        problems.push(format!(
            "the Crossref snapshot has {} records but the registry has {seen} DOIs",
            snapshot.len()
        ));
    }
    problems
}

/// Each schema's `schema_version` constant must match the code that writes it.
fn schema_versions(root: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for (file, version) in [
        ("results.schema.json", dima_core::model::SCHEMA_VERSION),
        ("hcs.schema.json", dima_core::model::HCS_SCHEMA_VERSION),
        (
            "run-report.schema.json",
            dima_core::run_report::REPORT_SCHEMA_VERSION,
        ),
    ] {
        let path = root.join("schemas").join(file);
        let schema: Option<Value> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());
        let constant = schema
            .as_ref()
            .and_then(|s| s.pointer("/properties/schema_version/const"))
            .and_then(Value::as_str);
        if constant != Some(version) {
            problems.push(format!(
                "schemas/{file}: schema_version const is {constant:?}, the code writes {version:?}"
            ));
        }
    }
    problems
}

/// `CHANGELOG.md` must have a section for this version (Keep a Changelog 1.1.0; D68).
fn changelog(root: &Path) -> Vec<String> {
    let version = citations::SOFTWARE_VERSION;
    match std::fs::read_to_string(root.join("CHANGELOG.md")) {
        Ok(text)
            if text
                .lines()
                .any(|l| l.starts_with(&format!("## [{version}]"))) =>
        {
            Vec::new()
        }
        Ok(_) => vec![format!("CHANGELOG.md has no `## [{version}]` section")],
        Err(e) => vec![format!("CHANGELOG.md: {e}")],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stale_versions_are_reported() {
        let root = std::env::temp_dir().join(format!("dima-xtask-versions-{}", std::process::id()));
        let files = [
            "docs/src/content/docs/getting-started/install.mdx",
            "docs/src/content/docs/reference/citing.mdx",
            "README.md",
            "packaging/bioconda/meta.yaml",
        ];
        let current = dima_core::citations::SOFTWARE_VERSION;
        for f in files {
            let path = root.join(f);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, format!("VERSION={current}\nDiMA v5.0.9 dima-perf\n")).unwrap();
        }
        assert!(super::release_versions(&root).is_empty());
        std::fs::write(
            root.join("README.md"),
            "The commands below use version\n0.9.0; cargo install --tag v0.9.0 dima\n",
        )
        .unwrap();
        let problems = super::release_versions(&root);
        assert_eq!(problems.len(), 2, "{problems:?}");
        for f in files {
            std::fs::remove_file(root.join(f)).unwrap();
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn tier_tags_are_checked() {
        use super::tag_problems;
        assert!(tag_problems("Text [@list2017, Rule 5]; [@miller1968] [general].").is_empty());
        assert!(tag_problems("**[general]** explains [@wcag22, SC 1.4.1] [standard].").is_empty());
        let p = tag_problems("[@miller1968] and [@list2017] [general], then [standard].");
        assert_eq!(p.len(), 3, "{p:?}");
        assert!(tag_problems("[@list2017; @miller1968] [general]")[0].contains("mixes tiers"));
        assert!(tag_problems("[@nosuchkey2000]")[0].contains("not in the registry"));
    }
}
