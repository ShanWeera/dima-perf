#!/bin/bash
set -euxo pipefail

# Licences of every compiled dependency, shipped with the package (about.license_file).
cargo-bundle-licenses --format yaml --output THIRDPARTY.yml

# The command-line tool only (the desktop app is distributed separately).
cargo install --no-track --locked --verbose --root "${PREFIX}" --path crates/dima
