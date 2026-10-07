# Bioconda recipe template

A ready-to-submit [Bioconda](https://bioconda.github.io/) recipe for the `dima` command-line tool
(Grüning et al. 2018, *Nature Methods* 15:475–476, doi:10.1038/s41592-018-0046-7). Once
accepted, Bioconda also builds a BioContainers Docker image and an Apptainer/Singularity image
automatically (da Veiga Leprevost et al. 2017, doi:10.1093/bioinformatics/btx192).

## Submitting a release

1. Tag and publish the release (the release workflow does everything else).
2. Fill in `version` and `sha256` in `meta.yaml`:
   ```sh
   curl -sL https://github.com/ShanWeera/dima-perf/archive/refs/tags/vX.Y.Z.tar.gz | sha256sum
   ```
3. Fork [bioconda/bioconda-recipes](https://github.com/bioconda/bioconda-recipes), copy
   `meta.yaml` and `build.sh` to `recipes/dima/`, and lint and build locally:
   ```sh
   bioconda-utils lint --packages dima
   bioconda-utils build --docker --mulled-test --packages dima
   ```
4. Open a pull request against bioconda-recipes. After the first release, Bioconda's bot
   proposes version updates automatically.

The recipe builds from source with conda's Rust and C compilers (the C compiler builds the
bundled zstd library), bundles the licences of all compiled dependencies, and its test runs a
real analysis and compares the result with the repository's golden output byte for byte.
