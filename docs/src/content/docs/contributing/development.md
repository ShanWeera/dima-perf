---
title: Development setup
description: Build, run and check DiMA from source — the Rust workspace, the generated files, the reference implementation and the documentation site.
---

The repository holds the library, the command line, the desktop app, the generators of the
documentation and citation files, the independent reference implementation used as a test
oracle, the test datasets and this documentation site. The working agreements for every change
are in [Working agreements](/contributing/working-agreements/) (the repository's `AGENTS.md`).

## Requirements

- **Rust** through [rustup](https://rustup.rs/): the repository's `rust-toolchain.toml` selects
  the stable toolchain with `clippy` and `rustfmt`. The minimum supported versions are 1.85 for
  the library and the command line and 1.95 for the desktop app, and both are tested.
- **Desktop app on Linux**: the X11, Wayland and Vulkan development packages listed in
  `scripts/ci/install-gui-deps.sh`.
- **Python 3.10 or newer**, standard library only, for the reference implementation.
- **Documentation**: Node.js 24 (the version in `docs/.node-version`) and pnpm (the exact version
  is pinned in `docs/package.json`; `corepack enable` provides it).

## Build and run

```sh
git clone https://github.com/ShanWeera/dima-perf
cd dima-perf
cargo build                                  # library and command line (debug)
cargo run -p dima -- analyze -i samples/mers_spike_aa.fasta -o /tmp/mers.json
cargo run -p dima-gui                        # the desktop app
cargo build --release -p dima                # an optimised binary in target/release/
```

A bare `cargo build` or `cargo test` covers the library and the command line; the desktop app is
built with `-p dima-gui` or `--workspace`.

## Before every change

Run, in this order (`cargo fmt` first, because clippy checks the formatted code):

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
RUSTUP_TOOLCHAIN=1.85 cargo test -p dima-core -p dima --locked     # minimum Rust, core and CLI
RUSTUP_TOOLCHAIN=1.95 cargo check -p dima-gui --all-targets --locked  # minimum Rust, desktop app
cargo xtask gen --mode check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p dima-core -p dima --locked  # API docs
cargo bench --no-run -p dima-core --locked                         # benchmarks compile
python3 scripts/reference/dima_reference.py --self-test
pnpm -C docs install --frozen-lockfile && pnpm -C docs build       # after documentation changes
```

## Generated files

Some files are generated from the code and must never be edited by hand:

| File | Generated from |
|---|---|
| `docs/src/content/docs/reference/cli.md` | the command-line definitions (`crates/dima/src/cli`) |
| `docs/src/content/docs/reference/bibliography.md`, `docs/src/references.json`, `docs/src/references.bib` | the citation registry (`crates/dima-core/src/citations.rs`) |
| `CITATION.cff`, `.zenodo.json` | the citation registry and the workspace version |
| `README.md` (its citation section only) | the citation registry |
| `THIRD-PARTY-NOTICES.md` | the dependency tree (`python3 scripts/notices/generate.py`, with cargo-about) |

```sh
cargo xtask gen --mode write    # regenerate after changing options, help texts or references
cargo xtask gen --mode check    # what CI runs: fails if a generated file is out of date
```

Check mode also compares the citation registry with a stored Crossref snapshot
(`xtask/data/crossref_snapshot.json`); `scripts/citations/crossref.py` fetches every DOI from
Crossref again and reports where the snapshot differs (it changes nothing: check each difference,
then update the snapshot and the registry by hand).

## Adding a reference

Every citation — in help texts, the desktop app's tooltips, the results' `run.references` and
these pages — comes from `crates/dima-core/src/citations.rs` (records with a DOI live in
`citations_data.rs`, sorted by key). Add the record there with its tier (domain, general or
standard; see the [citation policy](/contributing/working-agreements/)), run
`cargo xtask gen --mode write`, then cite it in the documentation by its key, for example
`[@chao2013]`, with a locator, `[@list2017, Rule 5]`, or several works at once,
`[@seemann2013; @list2017, Rule 5]`, and end the page with a `## References` heading followed by
a paragraph that contains only `[^ref]`. The site renders citations and the page's reference
list from `docs/src/references.json`, so their texts are those of `dima cite` (led by a
standard's designation, and with a year letter where the label has one) and of the Bibliography
page (`docs/plugins/rehype-registry-citations.mjs`). The build fails on an unknown
key, an empty locator, a citation inside a link, and a page with citations but no `[^ref]`, or
the reverse.

## The documentation site

```sh
pnpm -C docs install --frozen-lockfile
pnpm -C docs dev                 # live preview at http://localhost:4321/dima-perf/latest/
pnpm -C docs run check           # type and content checks
pnpm -C docs run build           # the site in docs/dist/, with internal link validation
pnpm -C docs run test:a11y       # WCAG 2.2 AA scan of every page (needs Chrome or Chromium)
pnpm -C docs run test:lighthouse # Lighthouse budget (docs/lighthouserc.cjs)
```

The site is built with [Starlight](https://starlight.astro.build/). Pages are Markdown or MDX in
`docs/src/content/docs/`; link to other pages by their path without the base, such as
`/reference/cli/`, and the build adds the base path of the version being built (`DOCS_BASE`,
default `/dima-perf/latest/`). Formulas are written in TeX between `$…$` and `$$…$$`, numbered
with `\label{…}` and referenced with `$\eqref{…}$`; they are rendered by MathJax with MathML for
screen readers. The accessibility and Lighthouse checks use an installed Chrome, or the browser
named by `CHROME_PATH`.

## Making a release

1. Set the new version in the workspace `Cargo.toml` (`[workspace.package] version`, following
   [Semantic Versioning](https://semver.org/spec/v2.0.0.html)) and run `cargo build` to update
   `Cargo.lock`.
2. Give the release a dated section in `CHANGELOG.md` (`## [X.Y.Z] - YYYY-MM-DD`); the release
   notes are taken from it.
3. Run `cargo xtask gen --mode write` (`CITATION.cff`, `.zenodo.json` and the README citation
   carry the version), then `cargo xtask gen --mode check`: it also lists every hand-written
   version number to update, in the install page, the citing page, the README and the Bioconda
   recipe.
4. Run the checks of [Before every change](#before-every-change), commit, and merge into
   `master`.
5. Start the release: on GitHub, open **Actions**, choose **Release**, then **Run workflow** on
   `master` and enter the version (`X.Y.Z`). Pushing a tag `vX.Y.Z` does the same. The release
   workflow refuses a version that does not match the workspace version, has no dated changelog
   section or already has a release, runs the full CI suite, builds and checks every package,
   and publishes the release with its checksums and attestations. A run started from the
   Actions page releases the newest `master` commit at the time it starts, and creates the tag
   `vX.Y.Z` on it only after every build and check has passed, so a failed run can simply be
   started again. The documentation of the new version is then published at `/dima-perf/vX.Y/`.
   Do not create the release on GitHub's release page: the workflow creates it.

## Other scripts

| Script | Purpose |
|---|---|
| `scripts/validate_paper.sh` | Prints the comparison with the paper's published values ([Comparison with the paper](/explanation/paper-comparison/)) |
| `scripts/bench/run.sh` | Regenerates `BENCHMARK.md` ([Benchmarks](/explanation/benchmarks/)) |
| `scripts/reference/dima_reference.py` | The independent reference implementation ([Testing](/contributing/testing/)) |
