# AGENTS.md — working agreements for humans and AI agents

This file is the single source of instructions for anyone (human or AI agent) working on this
repository. `CLAUDE.md` imports it; other agents (Cursor, Codex, Copilot, …) read it directly.

## What this repository is

`dima` is a Rust re-implementation of **DiMA: sequence diversity dynamics analyser for viruses**
(Tharanga S, Ünlü ES, Hu Y, Sjaugi MF, Çelik MA, Hekimoğlu H, Miotto O, Öncel MM, Khan AM.
*Brief Bioinform* 2025;26(1):bbae607, published online 26 Nov 2024, doi:10.1093/bib/bbae607).

Workspace layout:

| Path | Contents |
|---|---|
| `crates/dima-core` | Library `dima_core` (`#![forbid(unsafe_code)]`, no terminal dependencies) |
| `crates/dima` | CLI binary `dima` |
| `crates/dima-gui` | Desktop app `dima-gui` (egui/eframe) |
| `xtask` | `cargo xtask gen --mode write\|check` (generated docs, schemas, citations) |
| `schemas/` | JSON Schemas (Draft 2020-12) for every JSON output |
| `scripts/reference/` | Independent stdlib-only Python reference implementation (test oracle) |
| `tests/data/` | Committed datasets with provenance (see `tests/data/README.md`) |
| `docs/` | Astro Starlight documentation site |

## Working agreements

1. **The paper is validated.** DiMA is published in a peer-reviewed journal, so its methodology is
   the specification. Any change to the methodology needs extremely strong justification from
   highly cited, reputable, relevant peer-reviewed sources, and those citations must appear in
   every relevant place (CLI help, docs, rustdoc, GUI tooltips). Pure engineering changes must
   leave results unchanged.
2. **"Not sure" means research more.** When the maintainer answers a question with "Not sure",
   research deeper (literature, domain tools, measurements) and come back, through the question
   tool, with the best options and a recommendation to choose from. It never means skipping the
   question or assuming an answer: never treat it as approval and never decide on their behalf.
   Each refined question says in its own text that it follows the additional research the
   maintainer asked for (and which research round it is), so it is never mistaken for a question
   asked for the first time.
3. **Re-checks are always full.** When asked to check everything again, do it again from scratch,
   with fresh eyes, every time.
4. **Ask, don't assume.** Decisions that are not covered by the specification or by an existing
   decision are asked, not guessed.
5. **Quality bar for every change:** robust, resilient, maintainable, scalable, free of bugs and
   edge cases, single-responsibility with separated concerns, does not break existing
   functionality, scientifically sound, extremely performant with a low resource footprint
   without over-complication, user-friendly with a polished UI/UX, and easy to install.
6. **Subagents** run on the most capable model at maximum effort. Their prompts must forbid
   sending the maintainer's email or any personal data to external services (for example in
   HTTP `User-Agent` headers) and must carry agreement 8; subagents return the list of papers
   they could not read, each with its DOI (or full reference, if it has none) and what is
   needed from it.
7. **Every user-facing change is documented, fully, in the same pull request.** Anything a
   user can see or use (an option, default, output key or column, message, warning,
   behaviour, result, or desktop-app control) is documented on every page that describes
   it before the pull request is done. User-facing documentation is comprehensive and easy
   to read and follow: plain words, short sentences, every term defined at first use or
   linked to the Glossary, what the user does in the order they do it, what each result
   means and when to be careful. New or changed options, outputs and metrics get a worked
   example with real output; messages and warnings get their cause and fix; desktop-app
   changes get the steps; fixes get a CHANGELOG line. The documentation never assumes
   knowledge the page does not give or link. A change whose documentation is incomplete or
   hard to follow is not finished; completeness is never traded for speed.
8. **A paper that cannot be read is asked for, never guessed.** When a source needed for
   a decision or a citation cannot be found or read in full for any reason (paywall,
   blocked or failed download, no accessible copy) after every legitimate route has been
   tried (publisher, Europe PMC, PubMed Central, the authors' or institution's copy), give
   the maintainer its DOI (or full reference, if it has none) and what is needed from it;
   the maintainer supplies the PDF. The version of record or the authors' accepted
   manuscript, from the publisher, PMC/Europe PMC or an author's or institutional
   repository, counts, with the version stated; a submitted preprint does not. Never
   through shadow libraries. Until then nothing rests on it: an abstract, a secondary
   summary or a remembered claim never stands in for the text, and a source is never cited
   for anything not read in it.
9. **Peer-reviewed sources are for the methodology.** Peer-reviewed references and
   citations are needed only for the science of dima's methodology: what is computed, how,
   and how its results are read. Software-engineering choices (UI/UX, architecture,
   performance, testing, documentation tooling) need none and are never held up on
   literature research.

## Documentation checklist (every user-facing change)

| Change | Pages to update |
|---|---|
| New or changed option | CLI reference (generated), how-to guide that uses it, desktop-app guide (if in the app), FAQ if it answers a common question |
| New or changed output | JSON and TSV reference, Glossary, how-to "Choose and read outputs", tutorial if it shows that output |
| New or changed method or metric | Methodology page (formulas, worked example, limitations, sources), Comparison with the papers (the reproducible published values of the method's source papers), Differences, Numerical accuracy if numerics change, README and docs home feature summaries, CHANGELOG "Methodology" |
| New message or warning | Diagnostics / Troubleshooting with cause and fix |
| Desktop-app change | Desktop-app guide, Design rationale (the UX choice and its reason) |
| Engineering choice | Design rationale, Architecture, Testing if tests change |

Every row also gets a CHANGELOG entry.

## Citation policy (tiered, domain first)

* **Tier 1 — domain:** peer-reviewed work in bioinformatics, computational biology, virology,
  microbiology, ecology and evolution, or biostatistics venues (Bioinformatics, NAR, Brief
  Bioinform, PLoS Comput Biol/Biol, Genome Biol, GigaScience, BMC Bioinformatics, Nat
  Methods/Biotechnol, Cell Systems, Front Bioinform, Virus Evol, Mol Biol Evol, Ecology, Methods
  Ecol Evol, Front Microbiol, …), and domain research in general journals (Nature, PNAS, PLoS
  ONE). Always cited first.
* **Tier 2 — `[general]`:** peer-reviewed HCI, visualisation, statistics, mathematics (including
  numerical analysis) or computer science, only when it is the primary source of a number,
  algorithm, theorem, finding or experiment that no domain source has. A classical mathematical result used by
  numerical code (an identity, expansion, convergence theorem or error bound) is cited to a
  peer-reviewed article that states or proves it in the form used, at the equation or theorem:
  the original when it is in English and readable, otherwise a later article.
* **Tier 3 — `[standard]`:** standards and specifications, cited for what they define: W3C,
  IETF, Unicode, ISO/IEC and BIPM (SI Brochure) documents, and the specification of a file or
  data format by whoever maintains it, including the documentation in which a database or tool
  defines the symbols of the files it distributes, cited only for those definitions and only
  where no peer-reviewed source documents them.
* Outside Tier 3, never cite blogs, vendor pages, preprints or other non-peer-reviewed sources.
* Cite the specific rule, figure or section (e.g. "List 2017, Rule 5"), and only for what the
  source actually says. Every DOI is checked in CI.
* Surfaces: CLI long help and GUI tooltips cite the sources behind that option's scientific
  behaviour; the docs methodology pages carry every scientific source; the docs *Design
  rationale* page carries any UX and engineering citations that exist (agreement 9: none are
  required), with tiers labelled; rustdoc carries
  algorithm origins. A numerical method that evaluates a published formula without changing it
  is an engineering choice: its sources go in rustdoc, the docs *Numerical accuracy* page and the
  *Design rationale* page, not in CLI help or tooltips. All of them are generated from
  `crates/dima-core/src/citations.rs`.

## Commands (run before declaring any work done)

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
RUSTUP_TOOLCHAIN=1.85 cargo test -p dima-core -p dima --locked     # MSRV (core + CLI)
RUSTUP_TOOLCHAIN=1.95 cargo check -p dima-gui --all-targets --locked  # MSRV (GUI)
cargo xtask gen --mode check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p dima-core -p dima --locked  # API docs build cleanly
cargo bench --no-run -p dima-core --locked                         # benchmarks compile
python3 scripts/reference/dima_reference.py --self-test
python3 scripts/release/check-windows-crt.py --self-test
python3 scripts/notices/generate.py --check                       # after dependency changes (cargo-about)
python3 scripts/reference/paper_values.py --check                 # after changes to the reference implementation or the datasets
cargo deny check bans licenses sources                             # after dependency changes (cargo-deny)
cargo deny --config deny-strict.toml --exclude dima-gui --exclude xtask --exclude-dev check bans  # cargo-deny 0.20+
pnpm -C docs install --frozen-lockfile && pnpm -C docs build       # after docs changes
```

Generated files are never edited by hand: `cargo xtask gen --mode write` regenerates the CLI
reference, the bibliography, the docs' citation data (`references.json`, `references.bib`),
`CITATION.cff`, `.zenodo.json` and the README's citation section; `python3 scripts/notices/generate.py` regenerates `THIRD-PARTY-NOTICES.md`; `python3 scripts/reference/paper_values.py` regenerates the paper test's expected values (`crates/dima-core/tests/paper_expected/mod.rs`) from the reference implementation.
`scripts/validate_paper.sh` compares results with the paper; `scripts/bench/run.sh` regenerates
`BENCHMARK.md` (run it on an otherwise idle machine).

`cargo fmt` first (it changes formatting that clippy then checks), then clippy, then tests.
Golden outputs are re-blessed with `DIMA_BLESS=1 cargo test` and reviewed in the git diff.

## Conventions

* Determinism: identical inputs produce byte-identical outputs on every platform, thread count
  and batch size. No randomness, no timestamps, no absolute paths in outputs. Logarithms come
  from `libm`.
* Errors carry a location (record, line, column, byte) and a hint, and map to the documented exit
  codes (0 ok, 1 internal, 2 usage, 3 I/O or system, 4 invalid input data, 130/143/129 signals).
* Messages go to stderr; data goes to stdout or files. Outputs are written atomically.
* No `unsafe` in `dima-core`. Platform `unsafe` lives only in the CLI's signal and resource code.
* Tests: unit tests next to the code, integration tests in `tests/`, property tests with
  `proptest`, golden files compared byte-for-byte, differential tests against the Python
  reference.
