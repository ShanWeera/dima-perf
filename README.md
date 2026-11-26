# DiMA: sequence diversity dynamics analyser for viruses

[![CI](https://github.com/ShanWeera/dima-perf/actions/workflows/ci.yml/badge.svg)](https://github.com/ShanWeera/dima-perf/actions/workflows/ci.yml)
[![Documentation](https://img.shields.io/badge/docs-shanweera.github.io%2Fdima--perf-blue)](https://shanweera.github.io/dima-perf/)
[![Paper](https://img.shields.io/badge/paper-10.1093%2Fbib%2Fbbae607-informational)](https://doi.org/10.1093/bib/bbae607)
[![Licence: MIT](https://img.shields.io/badge/licence-MIT-green)](LICENSE)

`dima` is a fast, deterministic implementation of **DiMA** (Tharanga et al. 2025, *Briefings in
Bioinformatics* 26(1):bbae607, [doi:10.1093/bib/bbae607](https://doi.org/10.1093/bib/bbae607)),
with a command-line tool and a desktop app. For every k-mer position of a viral protein or
nucleotide alignment it:

- counts the k-mers and their support, and labels positions without enough data (NS, LS, ELS);
- estimates Shannon entropy with the paper's sample-size correction (or an alternative
  estimator), and the sample coverage;
- optionally computes similarity-weighted entropy, in which near-identical variants count for
  less (Wu 2025, *Virus Evolution* 11(1):veaf029,
  [doi:10.1093/ve/veaf029](https://doi.org/10.1093/ve/veaf029)), an addition to the paper's
  method;
- classifies variants as Index, Major, Minor and Unique motifs with their incidences;
- optionally counts variants per value of FASTA header fields (host, country, year, …);
- finds historically conserved sequences, with a confidence bound for each.

Results are identical on every platform, thread count and run, and every methodological choice
is documented with its sources.

## Install

**Command line.** Download the archive for your system from the
[latest release](https://github.com/ShanWeera/dima-perf/releases/latest), unpack it and put
`dima` on your `PATH`:

| System | Archive |
|---|---|
| Linux x86_64 (glibc 2.17+: CentOS 7 and newer, Ubuntu, Debian, most HPC clusters) | `dima-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 (glibc 2.28+) | `dima-<version>-aarch64-unknown-linux-gnu.tar.gz` |
| Any Linux, static (Alpine, minimal containers) | `dima-<version>-{x86_64,aarch64}-unknown-linux-musl.tar.gz` |
| macOS (Apple silicon / Intel) | `dima-<version>-{aarch64,x86_64}-apple-darwin.tar.gz` |
| Windows (x86_64 / ARM64) | `dima-<version>-{x86_64,aarch64}-pc-windows-msvc.zip` |

Or build it from source with Rust 1.85 or newer:

```sh
cargo install --locked --git https://github.com/ShanWeera/dima-perf --tag v1.1.0 dima
```

A Bioconda package (and with it BioContainers Docker and Apptainer images) will follow; the
recipe is in [`packaging/bioconda`](packaging/bioconda).

**Desktop app.** The same release has a macOS disk image, Windows installers and Linux
AppImage, `.deb` and `.tar.gz` packages. They are not code-signed yet; the
[installation guide](https://shanweera.github.io/dima-perf/) explains the first launch.

## Quick start

```sh
# Analyse the MERS-CoV spike protein alignment of the paper (Fig. 1C): JSON results
dima analyze -i samples/mers_spike_aa.fasta -o mers.json

# The same as four tidy tables, plus the conserved sequences
dima analyze -i samples/mers_spike_aa.fasta --tsv-dir tables --hcs-output mers.hcs.json

# Metadata from '|'-separated headers, stratified by host and country
dima analyze -i alignment.fasta.gz --header-format 'Accession|Host|Country|Year' \
    --metadata-fields 'Host|Country' -o results.json.zst

# Convert results, recompute conserved sequences, cite the methods a run used
dima view -i mers.json -O tsv --table variants -o variants.tsv
dima hcs -i mers.json --hcs-min-incidence 95 -o conserved.tsv
dima cite --from mers.json
```

Input can be plain, gzip, bzip2 or zstd FASTA, from a file, a pipe or standard input.
`dima analyze --help` explains every option with its references; `-h` gives a summary.

## Outputs

- **JSON** with every position, the summaries and the full provenance of the run (parameters
  and where they came from, input checksum, alphabet detection, validation, diagnostics and
  references), compact or `--pretty`, optionally `.gz` or `.zst`;
- **JSONL**, one position per line;
- **TSV**: tidy `positions`, `variants`, `metadata` and `hcs` tables (`NA` for missing values);
- **conserved sequences** as JSON or TSV;
- an optional **run report** (`--run-report`) with times, resources and checksums, kept
  separate so the results stay byte-identical from run to run.

Every JSON output has a [JSON Schema](schemas/). Exit codes: 0 success, 1 internal error,
2 usage error, 3 input/output or system error, 4 invalid input data, 130/143/129 interrupted.

Numbers are written at full double precision. As in any floating-point program they carry a
small rounding error: about the 14th significant digit on alignments of thousands of sequences,
and no earlier than the 11th at 17.5 million sequences per position (the 10th for the opt-in
similarity-weighted entropy). That is at least 10¹⁰ times smaller than the sampling error
(10⁹ times for similarity-weighted entropy).
[Numerical accuracy](https://shanweera.github.io/dima-perf/latest/explanation/numerical-accuracy/)
has the measurements, and explains why a prototyped fix for the last digits is deferred.

## Documentation

<https://shanweera.github.io/dima-perf/>: installation, tutorials, how-to guides (metadata,
conserved sequences, HPC clusters, large alignments, reproducibility), the methodology with
every formula and source, a comparison with the paper's published results
([`scripts/validate_paper.sh`](scripts/validate_paper.sh) reproduces it), and the full
command-line and output reference.

## Getting help

- [Frequently asked questions](https://shanweera.github.io/dima-perf/latest/getting-started/faq/)
  and [Troubleshooting](https://shanweera.github.io/dima-perf/latest/how-to/troubleshooting/),
  which explains every error and warning.
- Questions and ideas: [GitHub Discussions](https://github.com/ShanWeera/dima-perf/discussions).
- Bugs: [GitHub Issues](https://github.com/ShanWeera/dima-perf/issues/new/choose).

## How to cite

<!-- cite:start -->
If you use DiMA, please cite the paper:

> Tharanga S, Ünlü ES, Hu Y, Sjaugi MF, Çelik MA, Hekimoğlu H, Miotto O, Öncel MM, Khan AM. DiMA: sequence diversity dynamics analyser for viruses. Briefings in Bioinformatics. 2025;26(1):bbae607. doi:10.1093/bib/bbae607

and the version of the software you used:

> Tharanga S. dima: sequence diversity dynamics analyser for viruses (Rust implementation), version 1.1.0 [software]. https://github.com/ShanWeera/dima-perf

`dima cite` prints both (`--format bibtex`, `ris` or `cff` for reference managers), and `dima cite --from results.json` adds a methods paragraph with the references of the methods that run used. [`CITATION.cff`](CITATION.cff) holds the same metadata for GitHub's "Cite this repository".

<details>
<summary>BibTeX</summary>

```bibtex
@article{tharanga2025,
  author    = {{Tharanga}, Shan and {Ünlü}, Eyyüb Selim and {Hu}, Yongli and {Sjaugi}, Muhammad Farhan and {Çelik}, Muhammet A and {Hekimoğlu}, Hilal and {Miotto}, Olivo and {Öncel}, Muhammed Miran and {Khan}, Asif M},
  title     = {{DiMA: sequence diversity dynamics analyser for viruses}},
  journal   = {Briefings in Bioinformatics},
  year      = {2025},
  volume    = {26},
  number    = {1},
  pages     = {bbae607},
  doi       = {10.1093/bib/bbae607},
}

@software{dima_rs_1_1_0,
  author    = {{Tharanga}, Shan},
  title     = {{dima: sequence diversity dynamics analyser for viruses (Rust implementation)}},
  version   = {1.1.0},
  url       = {https://github.com/ShanWeera/dima-perf},
  note      = {Implements the method of Tharanga et al. 2025, doi:10.1093/bib/bbae607},
}
```

</details>
<!-- cite:end -->

## Contributing

Working agreements, the quality bar and the commands to run before any change are in
[`AGENTS.md`](AGENTS.md). In short:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo xtask gen --mode check
```

## Licence

MIT ([`LICENSE`](LICENSE)). Third-party components and their licences, including the bundled
fonts and the test datasets, are listed in [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md)
and [`tests/data/README.md`](tests/data/README.md).
