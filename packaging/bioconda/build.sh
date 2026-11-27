#!/bin/bash
set -euxo pipefail

# A release build: `dima --help` and the messages link the documentation of this version
# (`.../vX.Y/`), not that of the development version (`.../latest/`; crates/dima-core/build.rs).
export DIMA_RELEASE=1

# Licences of every compiled dependency, shipped with the package (about.license_file).
cargo-bundle-licenses --format yaml --output THIRDPARTY.yml

# The command-line tool only (the desktop app is distributed separately).
cargo install --no-track --locked --verbose --root "${PREFIX}" --path crates/dima
